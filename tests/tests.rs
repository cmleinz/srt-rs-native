extern crate futures;
extern crate futures_timer;
extern crate rand;
extern crate srt;
extern crate tokio;
extern crate bytes;
extern crate simple_logger;
#[macro_use]
extern crate log;

use bytes::{BytesMut, Bytes};

use std::{cmp::Ordering, collections::BinaryHeap, time::{Duration, Instant}, io::{Error, ErrorKind}, str, fmt::Debug, thread};

use futures::{prelude::*, sync::mpsc, stream::iter_ok};

use rand::{thread_rng, distributions::{IndependentSample, Normal, Range}};

use futures_timer::{Delay, Interval};

use srt::{Sender, Receiver, DefaultSenderCongestionCtrl, DefaultReceiverCongestionCtrl, ConnectionSettings, SeqNumber, SocketID};

use tokio::executor::current_thread;

use log::LevelFilter;

struct LossyConn<T> {
    sender: mpsc::Sender<T>,
    receiver: mpsc::Receiver<T>,

    loss_rate: f64,
    delay_avg: Duration,
    delay_stddev: Duration,

    delay_buffer: BinaryHeap<TTime<T>>,
    delay: Delay,
}

struct TTime<T> {
    data: T,
    time: Instant,
}

impl<T> Ord for TTime<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.cmp(&self.time)
    }
}

impl<T> PartialOrd for TTime<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> PartialEq for TTime<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<T> Eq for TTime<T> {}

impl<T> Stream for LossyConn<T> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<T>, ()> {
        self.receiver.poll()
    }
}

impl<T: Debug> Sink for LossyConn<T> {
    type SinkItem = T;
    type SinkError = ();

    fn start_send(&mut self, to_send: T) -> StartSend<T, ()> {

        // should we drop it?
        {
            let between = Range::new(0f64, 1f64);
            let sample = between.ind_sample(&mut thread_rng());

            if sample < self.loss_rate {
                info!("Dropping packet: {:?}", to_send);

                // drop
                return Ok(AsyncSink::Ready);
            }
        }

        // delay
        {
            let center =
                self.delay_avg.as_secs() as f64 + self.delay_avg.subsec_nanos() as f64 / 1e9;
            let stddev =
                self.delay_stddev.as_secs() as f64 + self.delay_stddev.subsec_nanos() as f64 / 1e9;

            let between = Normal::new(center, stddev);
            let delay_secs = between.ind_sample(&mut thread_rng());

            let delay = Duration::new(delay_secs.floor() as u64, ((delay_secs % 1.0) * 1e9) as u32);

            self.delay_buffer.push(TTime {
                data: to_send,
                time: Instant::now() + delay,
            })
        }

        // update the timer
        self.delay.reset_at(self.delay_buffer.peek().unwrap().time);

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), ()> {
        while let Async::Ready(_) = self.delay.poll().unwrap() {
            let val = match self.delay_buffer.pop() {
                Some(v) => v,
                None => break,
            };
            self.sender.start_send(val.data).unwrap(); // TODO: handle full

            // reset timer
            if let Some(i) = self.delay_buffer.peek() {
                self.delay.reset_at(i.time);
            }
        }

        Ok(self.sender.poll_complete().unwrap()) // TODO: not this
    }

    fn close(&mut self) -> Poll<(), ()> {
        Ok(self.sender.close().unwrap()) // TODO: here too
    }
}

impl<T> LossyConn<T> {
    fn new(
        loss_rate: f64,
        delay_avg: Duration,
        delay_stddev: Duration,
    ) -> (LossyConn<T>, LossyConn<T>) {
        let (a2b, bfroma) = mpsc::channel(10000);
        let (b2a, afromb) = mpsc::channel(10000);

        (
            LossyConn {
                sender: a2b,
                receiver: afromb,
                loss_rate,
                delay_avg,
                delay_stddev,

                delay_buffer: BinaryHeap::new(),
                delay: Delay::new_at(Instant::now()),
            },
            LossyConn {
                sender: b2a,
                receiver: bfroma,
                loss_rate,
                delay_avg,
                delay_stddev,

                delay_buffer: BinaryHeap::new(),
                delay: Delay::new_at(Instant::now()),
            },
        )
    }
}

struct CounterChecker {
    current: u64,
}

impl Sink for CounterChecker {
    type SinkItem = Bytes;
    type SinkError = Error;

    fn start_send(&mut self, by: Bytes) -> StartSend<Bytes, Error> {
        assert_eq!(str::from_utf8(&by[..]).unwrap(), self.current.to_string(), "Expected data to be {}, was {}", self.current, str::from_utf8(&by[..]).unwrap());

        if self.current % 10000 == 0 {
            println!("{} recognized", self.current);
        }
        self.current += 1;

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Error> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), Error> {
        self.poll_complete()
    }
}

#[test]
fn test_with_loss() {

    simple_logger::init().unwrap();
    log::set_max_level(LevelFilter::Info);

    const init_seq_num: u64 = 812731;
    const iters: u64 = 1_000_000;

    // a stream of ascending stringified integers
    let counting_stream =
        iter_ok((init_seq_num as u64..(init_seq_num + iters)))
            .map(|i| {
                BytesMut::from(&i.to_string().bytes().collect::<Vec<_>>()[..]).freeze()
            })
            .zip(Interval::new(Duration::new(0, 10)))
            .map(|(b, _)| b);

    let (send, recv) = LossyConn::new(0.2, Duration::from_secs(0), Duration::from_secs(0));

    let sender = Sender::new(
        send.map_err(|_| Error::new(ErrorKind::Other, "bad bad"))
            .sink_map_err(|_| Error::new(ErrorKind::Other, "bad bad")),
        DefaultSenderCongestionCtrl::new(),
        ConnectionSettings {
            init_seq_num: SeqNumber(812731),
            socket_start_time: Instant::now(),
            remote_sockid: SocketID(81),
            local_sockid: SocketID(13),
            max_packet_size: 1316,
            max_flow_size: 50_000,
            remote: "0.0.0.0:0".parse().unwrap(), // doesn't matter, it's getting discarded
        });

    let recvr = Receiver::new(
        recv.map_err(|_| Error::new(ErrorKind::Other, "bad bad"))
            .sink_map_err(|_| Error::new(ErrorKind::Other, "bad bad")),
        ConnectionSettings {
            init_seq_num: SeqNumber(812731),
            socket_start_time: Instant::now(),
            remote_sockid: SocketID(13),
            local_sockid: SocketID(81),
            max_packet_size: 1316,
            max_flow_size: 50_000,
            remote: "0.0.0.0:0".parse().unwrap(),
        });

    let t1 = thread::spawn(|| {
        counting_stream.forward(sender)
            .map_err(|e: Error| panic!("{:?}", e))
            .map(|_| ())
            .wait();
    });

    let t2 = thread::spawn(|| {
        recvr.forward(CounterChecker{current: init_seq_num})
            .map_err(|e| panic!(e))
            .map(move |(_, c)| assert_eq!(c.current, init_seq_num + iters))
            .wait();
    });

    t1.join();
    t2.join();
}
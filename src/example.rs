//! Mocks for doc examples
use crate::urc::URCMessages;
use atat::blocking::AtatClient;
use atat::{AtatCmd, AtatUrc, Error};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher};
use fugit::{TimerDurationU32, TimerInstantU32};
use fugit_timer::Timer;

/// ATAT client mock
pub struct ExampleAtClient<'a> {
    /// URC publisher used for statically mocking URC messages
    urc_publisher: Publisher<'a, CriticalSectionRawMutex, URCMessages<128>, 8, 1, 1>,
}

impl<'a> ExampleAtClient<'a> {
    pub fn urc_channel() -> PubSubChannel<CriticalSectionRawMutex, URCMessages<128>, 8, 1, 1> {
        PubSubChannel::new()
    }

    pub fn init(channel: &'a PubSubChannel<CriticalSectionRawMutex, URCMessages<128>, 8, 1, 1>) -> Self {
        Self {
            urc_publisher: channel.publisher().unwrap(),
        }
    }

    fn publish_urc(&self, message: &[u8]) {
        let message = URCMessages::parse(message).unwrap();
        self.urc_publisher.try_publish(message).unwrap();
    }
}

impl AtatClient for ExampleAtClient<'_> {
    fn send<A: AtatCmd>(&mut self, cmd: &A) -> Result<A::Response, Error> {
        let mut buffer = [0x0; 128];
        let length = cmd.write(&mut buffer);

        match &buffer[..length] {
            b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n" => {
                self.publish_urc(b"WIFI CONNECTED\r\n");
                self.publish_urc(b"WIFI GOT IP\r\n");
            }
            b"AT+CIPSTART=0,\"TCP\",\"10.0.0.1\",21\r\n" => self.publish_urc(b"0,CONNECT\r\n"),
            b"AT+CIPSEND=0,6\r\n" => {
                self.publish_urc(b"SEND OK\r\n");
                self.publish_urc(b"+IPD,0,16\r\n");
            }
            b"AT+CIPRECVDATA=0,64\r\n" => {
                self.publish_urc(b"+CIPRECVDATA,16:nice to see you!");
            }
            b"AT+CIPCLOSE=0\r\n" => self.publish_urc(b"0,CLOSED\r\n"),
            b"AT+CIFSR\r\n" => {
                let response = cmd
                    .parse(Ok(
                        b"+CIFSR:STAIP,\"10.0.0.181\"\r\n+CIFSR:STAMAC,\"10:fe:ed:05:ba:50\"\r\n",
                    ))
                    .map_err(|_| Error::Error)?;
                return Ok(response);
            }
            &_ => {}
        }

        let response = cmd.parse(Ok(b"\r\n")).map_err(|_| Error::Error)?;
        Ok(response)
    }
}

/// Timer mock
#[derive(Default)]
pub struct ExampleTimer {}

impl Timer<1_000_000> for ExampleTimer {
    type Error = u32;

    fn now(&mut self) -> TimerInstantU32<1000000> {
        unimplemented!()
    }

    fn start(&mut self, _duration: TimerDurationU32<1000000>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn wait(&mut self) -> nb::Result<(), Self::Error> {
        nb::Result::Err(nb::Error::WouldBlock)
    }
}

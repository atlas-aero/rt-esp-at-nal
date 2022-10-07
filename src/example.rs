//! Mocks for doc examples
use atat::{AtatClient, AtatCmd, AtatUrc, Error, Mode};
use fugit::{TimerDurationU32, TimerInstantU32};
use fugit_timer::Timer;
use heapless::Deque;

/// ATAT client mock
#[derive(Default)]
pub struct ExampleAtClient {
    /// Static URC messages
    urc_messages: Deque<&'static str, 2>,
}

impl AtatClient for ExampleAtClient {
    fn send<A: AtatCmd<LEN>, const LEN: usize>(&mut self, cmd: &A) -> nb::Result<A::Response, Error> {
        match cmd.as_bytes().as_slice() {
            b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n" => {
                self.urc_messages.push_back("WIFI CONNECTED\r\n").unwrap();
                self.urc_messages.push_back("WIFI GOT IP\r\n").unwrap();
            }
            b"AT+CIPSTART=0,\"TCP\",\"10.0.0.1\",21\r\n" => self.urc_messages.push_back("0,CONNECT\r\n").unwrap(),
            b"AT+CIPSEND=0,6\r\n" => {
                self.urc_messages.push_back("SEND OK\r\n").unwrap();
                self.urc_messages.push_back("+IPD,0,16\r\n").unwrap();
            }
            b"AT+CIPRECVDATA=0,64\r\n" => {
                self.urc_messages.push_back("+CIPRECVDATA,16:nice to see you!").unwrap();
            }
            b"AT+CIPCLOSE=0\r\n" => self.urc_messages.push_back("0,CLOSED\r\n").unwrap(),
            b"AT+CIFSR\r\n" => {
                let response = cmd
                    .parse(Ok(
                        b"+CIFSR:STAIP,\"10.0.0.181\"\r\n+CIFSR:STAMAC,\"10:fe:ed:05:ba:50\"\r\n",
                    ))
                    .map_err(|_| nb::Error::Other(Error::Error))?;
                return nb::Result::Ok(response);
            }
            &_ => {}
        }

        let response = cmd.parse(Ok(b"\r\n")).map_err(|_| nb::Error::Other(Error::Error))?;
        nb::Result::Ok(response)
    }

    fn peek_urc_with<URC: AtatUrc, F: FnOnce(URC::Response) -> bool>(&mut self, f: F) {
        if self.urc_messages.is_empty() {
            return;
        }

        if let Some(message) = URC::parse(self.urc_messages.pop_front().unwrap().as_bytes()) {
            f(message);
        }
    }

    fn check_response<A: AtatCmd<LEN>, const LEN: usize>(&mut self, _cmd: &A) -> nb::Result<A::Response, Error> {
        nb::Result::Err(nb::Error::WouldBlock)
    }

    fn get_mode(&self) -> Mode {
        Mode::Timeout
    }

    fn reset(&mut self) {}
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

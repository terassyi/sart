use crate::bgp::error::Error;
use crate::bgp::event::{
    AdministrativeEvent, BgpMessageEvent, Event, TcpConnectionEvent, TimerEvent,
};

#[derive(Debug)]
pub(crate) struct FiniteStateMachine {
    state: State,
}

impl FiniteStateMachine {
    pub fn get_state(&self) -> State {
        self.state
    }

    // https://www.rfc-editor.org/rfc/rfc4271#section-8.2.2
    pub fn mv(&mut self, event: Event) {
        match self.state {
			State::Idle => {
				match event {
					Event::Admin(e) => {
						match e {
							AdministrativeEvent::ManualStart | AdministrativeEvent::AutomaticStart => self.state = State::Connect,
							AdministrativeEvent::ManualStop | AdministrativeEvent::AutomaticStop => {},
							AdministrativeEvent::ManualStartWithPassiveTcpEstablishment | AdministrativeEvent::AutomaticStartWithPassiveTcpEstablishment |
							AdministrativeEvent::AutomaticStartWithDampPeerOscillations | AdministrativeEvent::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment => self.state = State::Active,
						}
					}
					_ => {},
				}
			},
			State::Connect => {
				match event {
					Event::Admin(e) => {
						match e {
							AdministrativeEvent::ManualStop | AdministrativeEvent::AutomaticStop => self.state = State::Idle,
							_ => {},
						}
					},
					Event::Timer(e) => {
						match e {
							TimerEvent::ConnectRetryTimerExpire => {},
							TimerEvent::DelayOpenTimerExpire => self.state = State::OpenSent,
							_ => self.state = State::Idle,
						}
					},
					Event::Connection(e) => {
						match e {
							TcpConnectionEvent::TcpConnectionValid | TcpConnectionEvent::TcpCRInvalid => {},
							TcpConnectionEvent::TcpCRAcked | TcpConnectionEvent::TcpConnectionConfirmed => self.state = State::OpenSent,
							TcpConnectionEvent::TcpConnectionFail => self.state = State::Active,
						}
					},
					_ => self.state = State::Idle,
				}
			},
			State::Active => {
				match event {
					Event::Admin(e) => {
						match e {
							AdministrativeEvent::ManualStop | AdministrativeEvent::AutomaticStop => self.state = State::Idle,
							_ => {},
						}
					},
					Event::Timer(e) => {
						match e {
							TimerEvent::ConnectRetryTimerExpire => self.state = State::Connect,
							TimerEvent::DelayOpenTimerExpire => self.state = State::OpenSent,
							_ => self.state = State::Idle,
						}
					},
					Event::Connection(e) => {
						match e {
							TcpConnectionEvent::TcpCRAcked | TcpConnectionEvent::TcpConnectionConfirmed => self.state = State::OpenSent,
							TcpConnectionEvent::TcpConnectionFail => self.state = State::Idle,
							_ => {},
						}
					},
					Event::Message(e) => {
						match e {
							BgpMessageEvent::BgpOpen => self.state = State::OpenConfirm,
							_ => self.state = State::Idle,
						}
					},
				}
			},
			State::OpenSent => {
				match event {
					Event::Admin(e) => {
						match e {
							AdministrativeEvent::ManualStop | AdministrativeEvent::AutomaticStop => self.state = State::Idle,
							_ => {},
						}
					},
					Event::Connection(e) => {
						match e {
							TcpConnectionEvent::TcpCRInvalid => {},
							TcpConnectionEvent::TcpConnectionFail => self.state = State::Active,
							_ => {}, // tcp connection collision processing
						}
					},
					Event::Message(e) => {
						match e {
							BgpMessageEvent::BgpOpen => self.state = State::OpenConfirm,
							_ => self.state = State::Idle,
						}
					},
					_ => self.state = State::Idle,
				}
			},
			State::OpenConfirm => {
				match event {
					Event::Admin(e) => {
						match e {
							AdministrativeEvent::ManualStop | AdministrativeEvent::AutomaticStop => self.state = State::Idle,
							_ => {},
						}
					},
					Event::Timer(e) => {
						match e {
							TimerEvent::KeepaliveTimerExpire => {},
							_ => self.state = State::Idle,
						}
					},
					Event::Connection(e) => {
						match e {
							TcpConnectionEvent::TcpConnectionFail => self.state = State::Idle,
							TcpConnectionEvent::TcpCRInvalid => {}, // ignore the second connection tracking
							_ => {}, // track the second connection
						}
					},
					Event::Message(e) => {
						match e {
							BgpMessageEvent::BgpOpen => self.state = State::Idle, // collision detection
							BgpMessageEvent::KeepAliveMsg => self.state = State::Established,
							_ => self.state = State::Idle,
						}
					},
				}
			},
			State::Established => {
				match event {
					Event::Admin(e) => {
						match e {
							AdministrativeEvent::ManualStop | AdministrativeEvent::AutomaticStop => self.state = State::Idle,
							_ => {},
						}
					},
					Event::Timer(e) => {
						match e {
							TimerEvent::HoldTimerExpire => self.state = State::Idle,
							TimerEvent::KeepaliveTimerExpire => {},
							_ => self.state = State::Idle,
						}
					},
					Event::Connection(e) => {
						match e {
							TcpConnectionEvent::TcpConnectionValid => {}, // track the second connection
							TcpConnectionEvent::TcpCRInvalid => {},
							_ => self.state = State::Idle,
						}
					},
					Event::Message(e) => {
						match e {
							BgpMessageEvent::KeepAliveMsg | BgpMessageEvent::UpdateMsg => {},
							_ => self.state = State::Idle,
						}
					}
				}
			},
		}
    }
}

// https://www.rfc-editor.org/rfc/rfc4271#section-8.2.2
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum State {
    Idle,
    Connect,
    Active,
    OpenSent,
    OpenConfirm,
    Established,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bgp::event::{
        AdministrativeEvent, BgpMessageEvent, Event, TcpConnectionEvent, TimerEvent,
    };
    use rstest::rstest;
    #[rstest(
		init,
		input,
		expected,
		case(State::Idle, vec![Event::Message(BgpMessageEvent::UpdateMsg)], State::Idle),
		case(State::Idle, vec![Event::Admin(AdministrativeEvent::ManualStart), Event::Connection(TcpConnectionEvent::TcpCRAcked), Event::Message(BgpMessageEvent::BgpOpen), Event::Message(BgpMessageEvent::KeepAliveMsg)], State::Established),
		case(State::Idle, vec![Event::Admin(AdministrativeEvent::ManualStart), Event::Connection(TcpConnectionEvent::TcpCRAcked), Event::Message(BgpMessageEvent::BgpOpen), Event::Message(BgpMessageEvent::KeepAliveMsg), Event::Message(BgpMessageEvent::KeepAliveMsg)], State::Established),
		case(State::Idle, vec![Event::Admin(AdministrativeEvent::ManualStart), Event::Connection(TcpConnectionEvent::TcpCRAcked), Event::Message(BgpMessageEvent::BgpOpen), Event::Message(BgpMessageEvent::KeepAliveMsg), Event::Message(BgpMessageEvent::KeepAliveMsg), Event::Message(BgpMessageEvent::UpdateMsg)], State::Established),
		case(State::Idle, vec![Event::Admin(AdministrativeEvent::ManualStart), Event::Connection(TcpConnectionEvent::TcpCRAcked), Event::Message(BgpMessageEvent::BgpOpen), Event::Message(BgpMessageEvent::KeepAliveMsg), Event::Message(BgpMessageEvent::NotifMsg)], State::Idle),
		case(State::Idle, vec![Event::Admin(AdministrativeEvent::ManualStart), Event::Connection(TcpConnectionEvent::TcpCRAcked), Event::Admin(AdministrativeEvent::ManualStop)], State::Idle),
		case(State::Idle, vec![Event::Admin(AdministrativeEvent::ManualStart), Event::Connection(TcpConnectionEvent::TcpConnectionConfirmed), Event::Connection(TcpConnectionEvent::TcpConnectionFail)], State::Active),
		case(State::Active, vec![Event::Timer(TimerEvent::ConnectRetryTimerExpire)], State::Connect),

	)]
    fn works_fsm_mv(init: State, input: Vec<Event>, expected: State) {
        let mut fsm = FiniteStateMachine { state: init };
        for e in input.iter() {
            fsm.mv(*e);
        }
        assert_eq!(expected, fsm.get_state())
    }
}

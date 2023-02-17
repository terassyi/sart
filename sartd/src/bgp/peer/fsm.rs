use crate::bgp::event::Event;

#[derive(Debug)]
pub(crate) struct FiniteStateMachine {
    state: State,
}

impl FiniteStateMachine {
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    pub fn get_state(&self) -> State {
        self.state
    }

    // https://www.rfc-editor.org/rfc/rfc4271#section-8.2.2
    pub fn mv(&mut self, event: u8) -> State {
        let current = self.get_state();
        let new_state = match self.state {
			State::Idle => {
				match event {
					Event::AMDIN_MANUAL_START | Event::ADMIN_AUTOMATIC_START => State::Connect,
					Event::ADMIN_AUTOMATIC_START_WITH_PASSIVE_TCP_ESTABLISHMENT |
					Event::ADMIN_AUTOMATIC_START_WITH_DAMP_PEER_OSCILLATIONS |
					Event::ADMIN_AUTOMATIC_START_WITH_DAMP_PEER_OSCILLATIONS_AND_PASSIVE_TCP_ESTABLISHMENT | Event::ADMIN_MANUAL_START_WITH_PASSIVE_TCP_ESTABLISHMENT => State::Active,
					_ => current,
				}
			},
			State::Connect => {
				match event {
					Event::TIMER_DELAY_OPEN_TIMER_EXPIRE |
					Event::CONNECTION_TCP_CR_ACKED | Event::CONNECTION_TCP_CONNECTION_CONFIRMED => State::OpenSent,
					Event::CONNECTION_TCP_CONNECTION_FAIL => State::Active,
					1 | 3 | 4 | 5 | 6 | 7 | 9 | 14 | 15 => current,
					_ => State::Idle,
				}
			},
			State::Active => {
				match event {
					Event::TIMER_CONNECT_RETRY_TIMER_EXPIRE => State::Connect,
					Event::TIMER_DELAY_OPEN_TIMER_EXPIRE |
					Event::CONNECTION_TCP_CR_ACKED | Event::CONNECTION_TCP_CONNECTION_CONFIRMED => State::OpenSent,
					Event::MESSAGE_BGP_OPEN => State::OpenConfirm,
					1 | 3 | 4 | 5 | 6 | 7 | 14 | 15 => current,
					_ => State::Idle,
				}
			},
			State::OpenSent => {
				match event {
					Event::CONNECTION_TCP_CONNECTION_FAIL => State::Active,
					Event::MESSAGE_BGP_OPEN => State::OpenConfirm,
					1 | 3 | 4 | 5 | 6 | 7 | 14 | 15 => current,
					_ => State::Idle,
				}
			},
			State::OpenConfirm => {
				match event {
					Event::MESSAGE_BGP_OPEN => State::Idle,
					Event::MESSAGE_KEEPALIVE_MSG => State::Established,
					1 | 3 | 4 | 5 | 6 | 7 | 14 | 15 | Event::TIMER_KEEPALIVE_TIMER_EXPIRE => current,
					_ => State::Idle,
				}
			},
			State::Established => {
				match event {
					Event::MESSAGE_KEEPALIVE_MSG | Event::MESSAGE_UPDATE_MSG => current,
					1 | 3 | 4 | 5 | 6 | 7 | 14 | 15 | Event::TIMER_KEEPALIVE_TIMER_EXPIRE => current,
					_ => State::Idle,
				}
			},
		};
        self.state = new_state;
        new_state
    }
}

// https://www.rfc-editor.org/rfc/rfc4271#section-8.2.2
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum State {
    Idle = 1,
    Connect = 2,
    Active = 3,
    OpenSent = 4,
    OpenConfirm = 5,
    Established = 6,
}

// impl From<State> for u32 {
// 	fn from(state: State) -> Self {
// 		match state {
// 			State::Idle => 1,
// 			State
// 		}
// 	}
// }

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    #[rstest(
		init,
		input,
		expected,
		case(State::Idle, vec![Event::MESSAGE_UPDATE_MSG], State::Idle),
		case(State::Idle, vec![Event::AMDIN_MANUAL_START, Event::CONNECTION_TCP_CR_ACKED, Event::MESSAGE_BGP_OPEN, Event::MESSAGE_KEEPALIVE_MSG], State::Established),
		case(State::Idle, vec![Event::AMDIN_MANUAL_START, Event::CONNECTION_TCP_CR_ACKED, Event::MESSAGE_BGP_OPEN, Event::MESSAGE_KEEPALIVE_MSG, Event::MESSAGE_KEEPALIVE_MSG], State::Established),
		case(State::Idle, vec![Event::AMDIN_MANUAL_START, Event::CONNECTION_TCP_CR_ACKED, Event::MESSAGE_BGP_OPEN, Event::MESSAGE_KEEPALIVE_MSG, Event::MESSAGE_KEEPALIVE_MSG, Event::MESSAGE_UPDATE_MSG], State::Established),
		case(State::Idle, vec![Event::AMDIN_MANUAL_START, Event::CONNECTION_TCP_CR_ACKED, Event::MESSAGE_BGP_OPEN, Event::MESSAGE_KEEPALIVE_MSG, Event::MESSAGE_NOTIF_MSG], State::Idle),
		case(State::Idle, vec![Event::AMDIN_MANUAL_START, Event::CONNECTION_TCP_CR_ACKED, Event::ADMIN_MANUAL_STOP], State::Idle),
		case(State::Idle, vec![Event::AMDIN_MANUAL_START, Event::CONNECTION_TCP_CONNECTION_CONFIRMED, Event::CONNECTION_TCP_CONNECTION_FAIL], State::Active),
		case(State::Active, vec![Event::TIMER_CONNECT_RETRY_TIMER_EXPIRE], State::Connect),

	)]
    fn works_fsm_mv(init: State, input: Vec<u8>, expected: State) {
        let mut fsm = FiniteStateMachine { state: init };
        for e in input.into_iter() {
            fsm.mv(e);
        }
        assert_eq!(expected, fsm.get_state())
    }
}

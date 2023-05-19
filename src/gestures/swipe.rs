use knuffel::{Decode, DecodeScalar};

#[derive(Decode, Debug, Clone, PartialEq, Eq)]
pub struct Swipe {
    #[knuffel(property)]
    pub direction: SwipeDir,
    #[knuffel(property)]
    pub fingers: i32,
    #[knuffel(property)]
    pub update: Option<String>,
    #[knuffel(property)]
    pub start: Option<String>,
    #[knuffel(property)]
    pub end: Option<String>,
    #[knuffel(property)]
    pub acceleration: Option<i8>,
    #[knuffel(property)]
    pub mouse_up_delay: Option<i64>,
}

/// Direction of swipe gestures
///
/// NW  N  NE
/// W   C   E
/// SW  S  SE
#[derive(DecodeScalar, Debug, Clone, PartialEq, Eq)]
pub enum SwipeDir {
    Any,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl SwipeDir {
    pub fn dir(x: f64, y: f64) -> SwipeDir {
        if x == 0.0 && y == 0.0 {
            return SwipeDir::Any;
        }

        let primary_direction = if x.abs() > y.abs() {
            if x < 0.0 { SwipeDir::W } else { SwipeDir::E }
        } else {
            if y < 0.0 { SwipeDir::N } else { SwipeDir::S }
        };

        let (ratio, secondary_direction) = match primary_direction {
            SwipeDir::N | SwipeDir::S => (x.abs() / y.abs(), if x < 0.0 { SwipeDir::W } else { SwipeDir::E }),
            SwipeDir::E | SwipeDir::W => (y.abs() / x.abs(), if y < 0.0 { SwipeDir::N } else { SwipeDir::S }),
            _ => (0.0, SwipeDir::Any),
        };

        if ratio > 0.4142 {
            match (primary_direction, secondary_direction) {
                (SwipeDir::N, SwipeDir::W) | (SwipeDir::W, SwipeDir::N) => SwipeDir::NW,
                (SwipeDir::N, SwipeDir::E) | (SwipeDir::E, SwipeDir::N) => SwipeDir::NE,
                (SwipeDir::S, SwipeDir::W) | (SwipeDir::W, SwipeDir::S) => SwipeDir::SW,
                (SwipeDir::S, SwipeDir::E) | (SwipeDir::E, SwipeDir::S) => SwipeDir::SE,
                _ => SwipeDir::Any,
            }
        } else {
            primary_direction
        }
    }
}

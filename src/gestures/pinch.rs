use knuffel::{Decode, DecodeScalar};

#[derive(Decode, Debug, Clone, PartialEq, Eq)]
pub struct Pinch {
    #[knuffel(property)]
    pub fingers: i32,
    #[knuffel(property)]
    pub direction: PinchDir,
    #[knuffel(property)]
    pub update: Option<String>,
    #[knuffel(property)]
    pub start: Option<String>,
    #[knuffel(property)]
    pub end: Option<String>,
}

/// Direction of pinch gestures
#[derive(DecodeScalar, Debug, Clone, PartialEq, Eq)]
pub enum PinchDir {
    In,
    Out,
    Clockwise,
    CounterClockwise,
    Any,
}

impl PinchDir {
    pub fn dir(scale: f64, delta_angle: f64) -> Self {
        // We have some rotation and very little scale
        if scale > 0.95 && scale < 1.05 && delta_angle.abs() > 0.03 {
            if delta_angle > 0.0 {
                Self::Clockwise
            } else {
                Self::CounterClockwise
            }
        // Otherwise we have a normal pinch
        } else if scale > 1.0 {
            Self::Out
        } else {
            Self::In
        }
    }
}

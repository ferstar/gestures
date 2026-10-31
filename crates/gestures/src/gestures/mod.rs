pub mod hold;
pub mod pinch;
pub mod swipe;

use knuffel::Decode;

use hold::Hold;
use pinch::Pinch;
use swipe::Swipe;

#[derive(Decode, Debug, Clone, PartialEq)]
pub enum Gesture {
    Swipe(Swipe),
    Pinch(Pinch),
    Hold(Hold),
    None,
}

use crate::gestures::Direction;

use super::*;

#[test]
fn test_config_default() {
    let c = Config::default();
    assert_eq!(
        c,
        Config {
            // device: None,
            gestures: vec![],
        }
    );
}

#[test]
fn test_direction_center() {
    assert_eq!(Direction::Any, Direction::dir(0.0, 0.0));
}

#[test]
fn test_direction_n() {
    assert_eq!(Direction::N, Direction::dir(0.0, -1.0));
}

#[test]
fn test_direction_s() {
    assert_eq!(Direction::S, Direction::dir(0.0, 1.0));
}

#[test]
fn test_direction_e() {
    assert_eq!(Direction::E, Direction::dir(1.0, 0.0));
}

#[test]
fn test_direction_w() {
    assert_eq!(Direction::W, Direction::dir(-1.0, 0.0));
}

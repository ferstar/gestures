use crate::{config::Config, gestures::Direction};

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
fn test_dir() {
    let test_cases = vec![
        (0.0, 0.0, Direction::Any),
        (1.0, 0.0, Direction::E),
        (-1.0, 0.0, Direction::W),
        (0.0, 1.0, Direction::S),
        (0.0, -1.0, Direction::N),
        (1.0, 1.0, Direction::SE),
        (-1.0, 1.0, Direction::SW),
        (1.0, -1.0, Direction::NE),
        (-1.0, -1.0, Direction::NW),
        (2.0, 1.0, Direction::SE),
        (-2.0, 1.0, Direction::SW),
        (2.0, -1.0, Direction::NE),
        (-2.0, -1.0, Direction::NW),
    ];

    for (x, y, expected) in test_cases {
        assert_eq!(Direction::dir(x, y), expected);
    }
}

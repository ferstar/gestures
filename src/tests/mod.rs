use crate::config::Config;
use crate::gestures::swipe::SwipeDir;

#[test]
fn test_config_default() {
    let c = Config::default();
    assert_eq!(
        c,
        Config {
            // // device: None,
            gestures: vec![],
        }
    );
}

#[test]
fn test_dir() {
    let test_cases = vec![
        (0.0, 0.0, SwipeDir::Any),
        (1.0, 0.0, SwipeDir::E),
        (-1.0, 0.0, SwipeDir::W),
        (0.0, 1.0, SwipeDir::S),
        (0.0, -1.0, SwipeDir::N),
        (1.0, 1.0, SwipeDir::SE),
        (-1.0, 1.0, SwipeDir::SW),
        (1.0, -1.0, SwipeDir::NE),
        (-1.0, -1.0, SwipeDir::NW),
        (2.0, 1.0, SwipeDir::SE),
        (-2.0, 1.0, SwipeDir::SW),
        (2.0, -1.0, SwipeDir::NE),
        (-2.0, -1.0, SwipeDir::NW),
    ];

    for (x, y, expected) in test_cases {
        assert_eq!(SwipeDir::dir(x, y), expected);
    }
}

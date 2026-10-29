use crate::config::Config;
use crate::gestures::swipe::SwipeDir;
use crate::utils::exec_command_from_string;

#[test]
fn test_zombie_process() {
    for _ in 0..100 {
        let _ = exec_command_from_string("echo", 0.0, 0.0, 0.0, 0.0);
    }
}

#[test]
fn test_config_default() {
    let c = Config::default();
    assert_eq!(
        c,
        Config {
            touchpad: None,
            gestures: vec![],
        }
    );
}

#[test]
fn generated_config_leaves_touchpad_acceleration_opt_in() {
    let config: Config = knuffel::parse("generated.kdl", crate::get_default_config()).unwrap();
    assert_eq!(config.touchpad, None);
    assert_eq!(config.gestures.len(), 1);
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

#[test]
fn swipe_direction_sector_boundaries() {
    use std::f64::consts::FRAC_PI_8;
    use SwipeDir::*;

    let sectors = [W, NW, N, NE, E, SE, S, SW, W];
    for (i, multiplier) in [-7.0, -5.0, -3.0, -1.0, 1.0, 3.0, 5.0, 7.0]
        .into_iter()
        .enumerate()
    {
        let boundary = multiplier * FRAC_PI_8;
        for (offset, expected) in [(-1e-10, &sectors[i]), (1e-10, &sectors[i + 1])] {
            let angle = boundary + offset;
            assert_eq!(SwipeDir::dir(angle.cos(), angle.sin()), *expected);
        }
    }
    assert_eq!(SwipeDir::dir(-0.0, -0.0), Any);
    assert_eq!(SwipeDir::dir(-1.0, -0.0), W);
    assert_eq!(SwipeDir::dir(f64::MIN_POSITIVE, 0.0), E);
}

#[test]
fn parses_swipe_pinch_and_hold_configuration() {
    use crate::gestures::{hold::Hold, pinch::Pinch, pinch::PinchDir, swipe::Swipe, Gesture};

    let config: Config = knuffel::parse(
        "test.kdl",
        r#"
swipe fingers=3 direction="any" mouse-up-delay=450 acceleration=20 start="start" update="move $delta_x $delta_y" end="end"
pinch fingers=2 direction="in" start="pinch-start" update="zoom $scale" end="pinch-end"
hold fingers=4 action="hold-action"
"#,
    )
    .unwrap();
    assert_eq!(
        config.gestures,
        [
            Gesture::Swipe(Swipe {
                fingers: 3,
                direction: SwipeDir::Any,
                mouse_up_delay: Some(450),
                acceleration: Some(20),
                start: Some("start".into()),
                update: Some("move $delta_x $delta_y".into()),
                end: Some("end".into()),
            }),
            Gesture::Pinch(Pinch {
                fingers: 2,
                direction: PinchDir::In,
                start: Some("pinch-start".into()),
                update: Some("zoom $scale".into()),
                end: Some("pinch-end".into()),
            }),
            Gesture::Hold(Hold {
                fingers: 4,
                action: Some("hold-action".into()),
            }),
        ]
    );
}

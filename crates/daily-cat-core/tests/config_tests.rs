use daily_cat_core::{
    AppConfig, Canvas, ConfigError, LayoutEngine, SafeArea, SourceError, SourcePlanner,
};

#[test]
fn app_config_accepts_cat_count_between_one_and_five() {
    for cat_count in 1..=5 {
        let config = AppConfig {
            cat_count,
            ..AppConfig::default()
        };

        assert_eq!(config.validate(), Ok(()));
    }
}

#[test]
fn app_config_rejects_zero_or_more_than_five_cats() {
    for cat_count in [0, 6, 9] {
        let config = AppConfig {
            cat_count,
            ..AppConfig::default()
        };

        assert_eq!(config.validate(), Err(ConfigError::InvalidCatCount));
    }
}

#[test]
fn source_planner_prefers_local_then_cataas_then_the_cat_api() {
    let planner = SourcePlanner {
        local_dirs: vec!["C:/cats".into()],
        cataas_enabled: true,
        the_cat_api_enabled: true,
    };

    assert_eq!(
        planner.ordered_sources().unwrap(),
        vec!["local:C:/cats", "cataas", "thecatapi"]
    );
}

#[test]
fn source_planner_errors_when_no_sources_are_enabled() {
    let planner = SourcePlanner {
        local_dirs: Vec::new(),
        cataas_enabled: false,
        the_cat_api_enabled: false,
    };

    assert_eq!(planner.ordered_sources(), Err(SourceError::NoSources));
}

#[test]
fn layout_engine_returns_one_slot_per_cat_inside_safe_area() {
    let engine = LayoutEngine;
    let slots = engine.slots(
        Canvas {
            width: 3840,
            height: 2160,
        },
        SafeArea {
            left: 420,
            right: 80,
            top: 80,
            bottom: 160,
        },
        3,
    );

    assert_eq!(slots.len(), 3);
    for slot in slots {
        assert!(slot.x >= 420);
        assert!(slot.y >= 80);
        assert!(slot.x + slot.width <= 3840 - 80);
        assert!(slot.y + slot.height <= 2160 - 160);
        assert!(slot.width > 0);
        assert!(slot.height > 0);
    }
}

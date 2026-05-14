use daily_cat_core::{
    AppConfig, Canvas, CatCountStrategy, ConfigError, ImageQuality, LanguagePreference,
    LayoutEngine, SafeArea, SourceError, SourcePlanner,
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
fn app_config_defaults_to_automatic_language_with_english_fallback() {
    let config = AppConfig::default();

    assert_eq!(config.language, LanguagePreference::Auto);
    assert_eq!(config.language.fallback_locale(), "en");
}

#[test]
fn app_config_defaults_to_display_matched_cat_count() {
    let config = AppConfig::default();
    let engine = LayoutEngine;

    assert_eq!(config.cat_count_strategy, CatCountStrategy::MatchDisplays);
    assert_eq!(engine.cat_assignments(2, &config), vec![0, 1]);
}

#[test]
fn fixed_single_cat_count_reuses_the_same_cat_on_each_display() {
    let config = AppConfig {
        cat_count_strategy: CatCountStrategy::Fixed,
        cat_count: 1,
        ..AppConfig::default()
    };
    let engine = LayoutEngine;

    assert_eq!(engine.cat_assignments(2, &config), vec![0, 0]);
}

#[test]
fn image_quality_rejects_sub_hd_thresholds() {
    let config = AppConfig {
        image_quality: ImageQuality {
            min_width: 1280,
            min_height: 720,
            preferred_width: 1920,
            preferred_height: 1080,
            allow_low_resolution_fallback: false,
        },
        ..AppConfig::default()
    };

    assert_eq!(config.validate(), Err(ConfigError::InvalidImageQuality));
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

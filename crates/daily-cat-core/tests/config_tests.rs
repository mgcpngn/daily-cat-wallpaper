use daily_cat_core::sources::{
    pixabay_search_query, the_cat_api_breed_ids, transparent_cat_prompt, wikimedia_search_query,
    SourceError,
};
use daily_cat_core::{
    AppConfig, Canvas, CatCountStrategy, ConfigError, ImageQuality, LanguagePreference,
    LayoutEngine, SafeArea, SourcePlanner,
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
fn image_quality_rejects_1080p_because_wallpapers_must_be_2k_or_better() {
    let config = AppConfig {
        image_quality: ImageQuality {
            min_width: 1920,
            min_height: 1080,
            preferred_width: 2560,
            preferred_height: 1440,
            allow_low_resolution_fallback: false,
        },
        ..AppConfig::default()
    };

    assert_eq!(config.validate(), Err(ConfigError::InvalidImageQuality));
}

#[test]
fn image_quality_rejects_low_resolution_fallback() {
    let config = AppConfig {
        image_quality: ImageQuality {
            allow_low_resolution_fallback: true,
            ..ImageQuality::default()
        },
        ..AppConfig::default()
    };

    assert_eq!(config.validate(), Err(ConfigError::InvalidImageQuality));
}

#[test]
fn source_planner_prefers_local_then_cataas_then_the_cat_api() {
    let planner = SourcePlanner {
        local_dirs: vec!["C:/cats".into()],
        wikimedia_commons_enabled: true,
        cataas_enabled: true,
        the_cat_api_enabled: true,
        ..SourcePlanner::default()
    };

    assert_eq!(
        planner.ordered_sources().unwrap(),
        vec!["local:C:/cats", "wikimedia", "thecatapi", "cataas"]
    );
}

#[test]
fn source_planner_errors_when_no_sources_are_enabled() {
    let planner = SourcePlanner {
        local_dirs: Vec::new(),
        wikimedia_commons_enabled: false,
        cataas_enabled: false,
        the_cat_api_enabled: false,
        ..SourcePlanner::default()
    };

    assert_eq!(planner.ordered_sources(), Err(SourceError::NoSources));
}

#[test]
fn breed_preferences_map_to_the_cat_api_ids() {
    let ids = the_cat_api_breed_ids(&[
        "british shorthair".to_string(),
        "ragdoll".to_string(),
        "maine coon".to_string(),
        "siamese".to_string(),
    ]);

    assert_eq!(ids, vec!["bsho", "ragd", "mcoo", "siam"]);
}

#[test]
fn wikimedia_query_uses_breed_and_image_mood() {
    let query = wikimedia_search_query(
        &["orange tabby".to_string()],
        &[daily_cat_core::config::CatImageType::Kitten],
    );

    assert!(query.contains("orange tabby cat"));
    assert!(query.contains("kitten"));
}

#[test]
fn advanced_api_keys_enable_premium_sources_before_public_sources() {
    let planner = SourcePlanner {
        pixabay_api_key: Some("pixabay-key".to_string()),
        magnific_api_key: Some("magnific-key".to_string()),
        ..SourcePlanner::default()
    };

    assert_eq!(
        planner.ordered_sources().unwrap(),
        vec!["magnific", "pixabay", "wikimedia", "thecatapi", "cataas"]
    );
}

#[test]
fn pixabay_query_is_wallpaper_oriented() {
    let query = pixabay_search_query(&["calico".to_string()], &[]);

    assert_eq!(query, "calico cat wallpaper");
}

#[test]
fn ai_prompt_requests_lifelike_transparent_desktop_pet_cutout() {
    let config = AppConfig {
        breeds: vec!["ragdoll".to_string()],
        image_types: vec![daily_cat_core::config::CatImageType::Healing],
        ..AppConfig::default()
    };

    let prompt = transparent_cat_prompt(&config);

    assert!(prompt.contains("transparent PNG cutout"));
    assert!(prompt.contains("computer desktop"));
    assert!(prompt.contains("complete body visible"));
    assert!(prompt.contains("No room"));
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

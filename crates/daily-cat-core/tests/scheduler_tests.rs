use chrono::{Local, TimeZone};
use daily_cat_core::config::ScheduleConfig;
use daily_cat_core::scheduler::RefreshTrigger;
use daily_cat_core::{RefreshDecision, Scheduler};

#[test]
fn manual_trigger_always_refreshes() {
    let scheduler = Scheduler::new(ScheduleConfig::ManualOnly);
    let now = Local.with_ymd_and_hms(2026, 5, 13, 9, 0, 0).unwrap();

    assert_eq!(
        scheduler.should_refresh(RefreshTrigger::Manual, now, None),
        RefreshDecision::Refresh
    );
}

#[test]
fn login_schedule_refreshes_on_app_launch() {
    let scheduler = Scheduler::new(ScheduleConfig::OnLogin);
    let now = Local.with_ymd_and_hms(2026, 5, 13, 9, 0, 0).unwrap();

    assert_eq!(
        scheduler.should_refresh(RefreshTrigger::AppLaunch, now, None),
        RefreshDecision::Refresh
    );
}

#[test]
fn daily_schedule_waits_until_configured_time() {
    let scheduler = Scheduler::new(ScheduleConfig::Daily {
        time: "09:00".to_string(),
    });
    let before = Local.with_ymd_and_hms(2026, 5, 13, 8, 59, 0).unwrap();
    let at_time = Local.with_ymd_and_hms(2026, 5, 13, 9, 0, 0).unwrap();
    let already_refreshed = Local.with_ymd_and_hms(2026, 5, 13, 9, 5, 0).unwrap();

    assert_eq!(
        scheduler.should_refresh(RefreshTrigger::Timer, before, None),
        RefreshDecision::Wait
    );
    assert_eq!(
        scheduler.should_refresh(RefreshTrigger::Timer, at_time, None),
        RefreshDecision::Refresh
    );
    assert_eq!(
        scheduler.should_refresh(RefreshTrigger::Timer, at_time, Some(already_refreshed)),
        RefreshDecision::Wait
    );
}

#[test]
fn interval_schedule_respects_last_refresh() {
    let scheduler = Scheduler::new(ScheduleConfig::EveryHours { hours: 4 });
    let last = Local.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap();
    let too_soon = Local.with_ymd_and_hms(2026, 5, 13, 11, 59, 0).unwrap();
    let ready = Local.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap();

    assert_eq!(
        scheduler.should_refresh(RefreshTrigger::Timer, too_soon, Some(last)),
        RefreshDecision::Wait
    );
    assert_eq!(
        scheduler.should_refresh(RefreshTrigger::Timer, ready, Some(last)),
        RefreshDecision::Refresh
    );
}

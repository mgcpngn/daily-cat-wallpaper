use crate::config::ScheduleConfig;
use chrono::{DateTime, Duration, Local, NaiveTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RefreshDecision {
    Refresh,
    Wait,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RefreshTrigger {
    AppLaunch,
    Timer,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scheduler {
    schedule: ScheduleConfig,
}

impl Scheduler {
    pub fn new(schedule: ScheduleConfig) -> Self {
        Self { schedule }
    }

    pub fn should_refresh(
        &self,
        trigger: RefreshTrigger,
        now: DateTime<Local>,
        last_refresh: Option<DateTime<Local>>,
    ) -> RefreshDecision {
        if trigger == RefreshTrigger::Manual {
            return RefreshDecision::Refresh;
        }

        match &self.schedule {
            ScheduleConfig::ManualOnly => RefreshDecision::Wait,
            ScheduleConfig::OnLogin => {
                if trigger == RefreshTrigger::AppLaunch {
                    RefreshDecision::Refresh
                } else {
                    RefreshDecision::Wait
                }
            }
            ScheduleConfig::Daily { time } => {
                if trigger != RefreshTrigger::Timer {
                    return RefreshDecision::Wait;
                }

                let Ok(target_time) = NaiveTime::parse_from_str(time, "%H:%M") else {
                    return RefreshDecision::Wait;
                };
                if now.time() < target_time {
                    return RefreshDecision::Wait;
                }
                if last_refresh.is_some_and(|last| last.date_naive() == now.date_naive()) {
                    return RefreshDecision::Wait;
                }

                RefreshDecision::Refresh
            }
            ScheduleConfig::EveryHours { hours } => {
                if trigger != RefreshTrigger::Timer {
                    return RefreshDecision::Wait;
                }

                let Some(last_refresh) = last_refresh else {
                    return RefreshDecision::Refresh;
                };

                if now.signed_duration_since(last_refresh) >= Duration::hours(i64::from(*hours)) {
                    RefreshDecision::Refresh
                } else {
                    RefreshDecision::Wait
                }
            }
        }
    }
}

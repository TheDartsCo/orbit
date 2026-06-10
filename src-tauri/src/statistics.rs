use crate::db::queries::StatisticsSessionRow;
use crate::models::{
    AgentStatisticsRow, ModelStatisticsRow, ProjectAgentShare, ProjectStatisticsCard,
    ProjectStatisticsRow, StatisticsDashboard, StatisticsMode, StatisticsPeriod,
    StatisticsSeriesValue, StatisticsSummary, StatisticsTimeBucket,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};

const UNKNOWN_PROJECT: &str = "Unknown project";
const OTHER: &str = "Other";
const MAX_CHART_CATEGORIES: usize = 8;

#[derive(Default)]
struct Aggregate {
    sessions: u64,
    messages: u64,
    tokens: u64,
    last_active: Option<DateTime<Utc>>,
}

#[derive(Clone)]
struct ProjectIdentity {
    key: String,
    display: String,
}

pub fn statistics_cutoff(period: StatisticsPeriod, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    match period {
        StatisticsPeriod::SevenDays => Some(start_of_day(now) - Duration::days(6)),
        StatisticsPeriod::ThirtyDays => Some(start_of_day(now) - Duration::days(29)),
        StatisticsPeriod::NinetyDays => Some(start_of_day(now) - Duration::days(89)),
        StatisticsPeriod::All => None,
    }
}

pub fn aggregate_statistics(
    mode: StatisticsMode,
    period: StatisticsPeriod,
    now: DateTime<Utc>,
    rows: &[StatisticsSessionRow],
) -> StatisticsDashboard {
    let cutoff = statistics_cutoff(period, now);
    let rows: Vec<_> = rows
        .iter()
        .filter(|row| cutoff.is_none_or(|cutoff| row.created_at >= cutoff))
        .cloned()
        .collect();
    let projects = project_identities(&rows);
    let summary = build_summary(&rows, &projects);

    match mode {
        StatisticsMode::Agent => build_agent_dashboard(summary, period, now, &rows),
        StatisticsMode::Project => build_project_dashboard(summary, period, now, &rows, &projects),
    }
}

fn build_summary(rows: &[StatisticsSessionRow], projects: &[ProjectIdentity]) -> StatisticsSummary {
    let sessions = rows.len() as u64;
    let messages = rows.iter().map(|row| row.message_count).sum();
    let total_tokens = rows.iter().map(session_tokens).sum();
    StatisticsSummary {
        sessions,
        messages,
        total_tokens,
        active_agents: rows
            .iter()
            .map(|row| row.agent.as_str())
            .collect::<HashSet<_>>()
            .len() as u64,
        project_count: projects
            .iter()
            .map(|project| project.key.as_str())
            .collect::<HashSet<_>>()
            .len() as u64,
        average_messages_per_session: divide(messages, sessions),
        average_tokens_per_session: divide(total_tokens, sessions),
    }
}

fn build_agent_dashboard(
    summary: StatisticsSummary,
    period: StatisticsPeriod,
    now: DateTime<Utc>,
    rows: &[StatisticsSessionRow],
) -> StatisticsDashboard {
    let mut agent_totals: HashMap<String, Aggregate> = HashMap::new();
    let mut model_totals: HashMap<String, u64> = HashMap::new();
    for row in rows {
        let agent = agent_totals.entry(row.agent.clone()).or_default();
        add_row(agent, row);
        *model_totals
            .entry(
                row.model
                    .as_deref()
                    .filter(|model| !model.trim().is_empty())
                    .unwrap_or(OTHER)
                    .to_string(),
            )
            .or_default() += session_tokens(row);
    }

    let mut agents: Vec<_> = agent_totals
        .into_iter()
        .map(|(agent, total)| AgentStatisticsRow {
            agent,
            sessions: total.sessions,
            messages: total.messages,
            tokens: total.tokens,
            average_messages: divide(total.messages, total.sessions),
            last_used: total.last_active.unwrap_or_else(epoch),
        })
        .collect();
    agents.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.agent.cmp(&b.agent)));

    let total_model_tokens: u64 = model_totals.values().sum();
    let model_totals = combine_other(model_totals, MAX_CHART_CATEGORIES);
    let mut models: Vec<_> = model_totals
        .into_iter()
        .map(|(model, tokens)| ModelStatisticsRow {
            model,
            tokens,
            percentage: percentage(tokens, total_model_tokens),
        })
        .collect();
    models.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.model.cmp(&b.model)));

    let agent_chart_totals = agents
        .iter()
        .map(|agent| (agent.agent.clone(), agent.sessions))
        .collect();
    let timeline_keys: Vec<_> = rows.iter().map(|row| row.agent.clone()).collect();
    let timeline = build_timeline(
        period,
        now,
        rows,
        &timeline_keys,
        agent_chart_totals,
        |_| 1,
        agent_label,
    );

    StatisticsDashboard::Agent {
        summary,
        timeline,
        agents,
        models,
    }
}

fn build_project_dashboard(
    summary: StatisticsSummary,
    period: StatisticsPeriod,
    now: DateTime<Utc>,
    rows: &[StatisticsSessionRow],
    identities: &[ProjectIdentity],
) -> StatisticsDashboard {
    let mut totals: HashMap<String, Aggregate> = HashMap::new();
    let mut displays: HashMap<String, (String, DateTime<Utc>)> = HashMap::new();
    let mut agent_totals: HashMap<String, HashMap<String, Aggregate>> = HashMap::new();

    for (row, identity) in rows.iter().zip(identities) {
        add_row(totals.entry(identity.key.clone()).or_default(), row);
        let display = displays
            .entry(identity.key.clone())
            .or_insert_with(|| (identity.display.clone(), row.updated_at));
        if row.updated_at >= display.1 {
            *display = (identity.display.clone(), row.updated_at);
        }
        add_row(
            agent_totals
                .entry(identity.key.clone())
                .or_default()
                .entry(row.agent.clone())
                .or_default(),
            row,
        );
    }

    let mut projects: Vec<_> = totals
        .into_iter()
        .map(|(key, total)| {
            let agents = agent_totals.remove(&key).unwrap_or_default();
            let agent_mix = project_agent_mix(agents);
            let top_agent = agent_mix
                .first()
                .map(|share| share.agent.clone())
                .unwrap_or_else(|| OTHER.to_string());
            ProjectStatisticsRow {
                project: displays
                    .get(&key)
                    .map(|display| display.0.clone())
                    .unwrap_or(key),
                sessions: total.sessions,
                messages: total.messages,
                tokens: total.tokens,
                agent_count: agent_mix.len() as u64,
                top_agent,
                last_active: total.last_active.unwrap_or_else(epoch),
                agent_mix,
            }
        })
        .collect();
    projects.sort_by(|a, b| {
        b.tokens
            .cmp(&a.tokens)
            .then_with(|| a.project.cmp(&b.project))
    });

    let cards = projects
        .iter()
        .take(4)
        .map(|project| ProjectStatisticsCard {
            project: project.project.clone(),
            sessions: project.sessions,
            tokens: project.tokens,
            last_active: project.last_active,
            agent_mix: project.agent_mix.clone(),
        })
        .collect();

    let project_totals: HashMap<_, _> = projects
        .iter()
        .map(|project| (project.project.clone(), project.tokens))
        .collect();
    let row_projects: Vec<_> = identities
        .iter()
        .map(|identity| {
            displays
                .get(&identity.key)
                .map(|display| display.0.clone())
                .unwrap_or_else(|| identity.display.clone())
        })
        .collect();
    let timeline = build_timeline(
        period,
        now,
        rows,
        &row_projects,
        project_totals,
        session_tokens,
        |label| label.to_string(),
    );

    StatisticsDashboard::Project {
        summary,
        timeline,
        projects,
        cards,
    }
}

fn project_agent_mix(agents: HashMap<String, Aggregate>) -> Vec<ProjectAgentShare> {
    let token_total: u64 = agents.values().map(|agent| agent.tokens).sum();
    let session_total: u64 = agents.values().map(|agent| agent.sessions).sum();
    let mut shares: Vec<_> = agents
        .into_iter()
        .map(|(agent, total)| {
            let basis = if token_total > 0 {
                total.tokens
            } else {
                total.sessions
            };
            let denominator = if token_total > 0 {
                token_total
            } else {
                session_total
            };
            ProjectAgentShare {
                agent,
                sessions: total.sessions,
                tokens: total.tokens,
                percentage: percentage(basis, denominator),
            }
        })
        .collect();
    shares.sort_by(|a, b| {
        b.tokens
            .cmp(&a.tokens)
            .then_with(|| b.sessions.cmp(&a.sessions))
            .then_with(|| a.agent.cmp(&b.agent))
    });
    shares
}

fn build_timeline<V, L>(
    period: StatisticsPeriod,
    now: DateTime<Utc>,
    rows: &[StatisticsSessionRow],
    row_keys: &[String],
    totals: HashMap<String, u64>,
    value_for_row: V,
    label_for_key: L,
) -> Vec<StatisticsTimeBucket>
where
    V: Fn(&StatisticsSessionRow) -> u64,
    L: Fn(&str) -> String,
{
    let categories = chart_categories(totals, MAX_CHART_CATEGORIES);
    let starts = bucket_starts(period, now, rows);
    let mut buckets: Vec<BTreeMap<String, u64>> = starts.iter().map(|_| BTreeMap::new()).collect();

    for (row, raw_key) in rows.iter().zip(row_keys) {
        let Some(index) = starts.iter().rposition(|start| row.created_at >= *start) else {
            continue;
        };
        let key = if categories.contains(raw_key) {
            raw_key.clone()
        } else {
            OTHER.to_string()
        };
        *buckets[index].entry(key).or_default() += value_for_row(row);
    }

    starts
        .into_iter()
        .zip(buckets)
        .map(|(start, values)| StatisticsTimeBucket {
            start,
            values: values
                .into_iter()
                .filter(|(_, value)| *value > 0)
                .map(|(key, value)| StatisticsSeriesValue {
                    label: label_for_key(&key),
                    key,
                    value,
                })
                .collect(),
        })
        .collect()
}

fn bucket_starts(
    period: StatisticsPeriod,
    now: DateTime<Utc>,
    rows: &[StatisticsSessionRow],
) -> Vec<DateTime<Utc>> {
    match period {
        StatisticsPeriod::SevenDays => daily_starts(now, 7),
        StatisticsPeriod::ThirtyDays => daily_starts(now, 30),
        StatisticsPeriod::NinetyDays => {
            let cutoff = start_of_day(now) - Duration::days(89);
            (0..13)
                .map(|index| cutoff + Duration::days(index * 7))
                .collect()
        }
        StatisticsPeriod::All => {
            let earliest = rows.iter().map(|row| row.created_at).min().unwrap_or(now);
            let mut year = earliest.year();
            let mut month = earliest.month();
            let mut starts = Vec::new();
            loop {
                starts.push(month_start(year, month));
                if year == now.year() && month == now.month() {
                    break;
                }
                if month == 12 {
                    year += 1;
                    month = 1;
                } else {
                    month += 1;
                }
            }
            starts
        }
    }
}

fn daily_starts(now: DateTime<Utc>, days: i64) -> Vec<DateTime<Utc>> {
    (0..days)
        .map(|index| start_of_day(now) - Duration::days(days - index - 1))
        .collect()
}

#[cfg(test)]
fn normalized_project_names(rows: &[StatisticsSessionRow]) -> Vec<String> {
    project_identities(rows)
        .into_iter()
        .map(|identity| identity.display)
        .collect()
}

fn project_identities(rows: &[StatisticsSessionRow]) -> Vec<ProjectIdentity> {
    let mut readable: Vec<(String, Vec<String>)> = rows
        .iter()
        .filter_map(|row| project_path_basename(&row.project_path))
        .map(|display| {
            let tokens = project_tokens(&display);
            (display, tokens)
        })
        .filter(|(_, tokens)| !tokens.is_empty())
        .collect();
    readable.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    readable.dedup_by(|a, b| a.1 == b.1);

    rows.iter()
        .map(|row| {
            let path = row.project_path.trim();
            if path.is_empty() || path == "-" {
                return ProjectIdentity {
                    key: "unknown-project".to_string(),
                    display: UNKNOWN_PROJECT.to_string(),
                };
            }

            if let Some(display) = project_path_basename(path) {
                return identity(display);
            }

            let path_tokens = project_tokens(path);
            if let Some((display, tokens)) = readable
                .iter()
                .find(|(_, tokens)| path_tokens.ends_with(tokens))
            {
                return ProjectIdentity {
                    key: tokens.join("-"),
                    display: display.clone(),
                };
            }

            if path.starts_with('-') {
                let display = path_tokens
                    .last()
                    .cloned()
                    .unwrap_or_else(|| UNKNOWN_PROJECT.to_string());
                return identity(display);
            }

            identity(path.to_string())
        })
        .collect()
}

fn project_path_basename(path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() || path == "-" {
        return None;
    }
    if path.contains('/') || path.contains('\\') {
        return path
            .split(['/', '\\'])
            .filter(|part| !part.is_empty())
            .next_back()
            .map(str::to_string);
    }
    None
}

fn identity(display: String) -> ProjectIdentity {
    let tokens = project_tokens(&display);
    if tokens.is_empty() {
        ProjectIdentity {
            key: "unknown-project".to_string(),
            display: UNKNOWN_PROJECT.to_string(),
        }
    } else {
        ProjectIdentity {
            key: tokens.join("-"),
            display,
        }
    }
}

fn project_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_lowercase())
        .collect()
}

fn add_row(total: &mut Aggregate, row: &StatisticsSessionRow) {
    total.sessions += 1;
    total.messages += row.message_count;
    total.tokens += session_tokens(row);
    if total
        .last_active
        .is_none_or(|last_active| row.updated_at > last_active)
    {
        total.last_active = Some(row.updated_at);
    }
}

fn session_tokens(row: &StatisticsSessionRow) -> u64 {
    row.input_tokens.saturating_add(row.output_tokens)
}

fn chart_categories(totals: HashMap<String, u64>, limit: usize) -> HashSet<String> {
    let mut totals: Vec<_> = totals.into_iter().collect();
    totals.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    totals.into_iter().take(limit).map(|(key, _)| key).collect()
}

fn combine_other(mut totals: HashMap<String, u64>, limit: usize) -> HashMap<String, u64> {
    if totals.len() <= limit {
        return totals;
    }
    let categories = chart_categories(totals.clone(), limit);
    let other: u64 = totals
        .extract_if(|key, _| !categories.contains(key))
        .map(|(_, value)| value)
        .sum();
    *totals.entry(OTHER.to_string()).or_default() += other;
    totals
}

fn agent_label(agent: &str) -> String {
    match agent {
        "opencode" => "OpenCode".to_string(),
        "jetbrains" => "JetBrains AI".to_string(),
        "antigravity" => "Antigravity".to_string(),
        other => {
            let mut characters = other.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                .unwrap_or_default()
        }
    }
}

fn divide(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        0
    } else {
        numerator / denominator
    }
}

fn percentage(value: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        value as f64 * 100.0 / total as f64
    }
}

fn start_of_day(value: DateTime<Utc>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &value
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("midnight is valid"),
    )
}

fn month_start(year: i32, month: u32) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(year, month, 1)
            .expect("valid month")
            .and_hms_opt(0, 0, 0)
            .expect("midnight is valid"),
    )
}

fn epoch() -> DateTime<Utc> {
    DateTime::from_timestamp(0, 0).expect("Unix epoch is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::StatisticsSessionRow;
    use crate::models::{StatisticsDashboard, StatisticsMode, StatisticsPeriod};
    use chrono::{TimeZone, Utc};

    fn timestamp(day: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, day, 12, 0, 0).unwrap()
    }

    fn row(
        agent: &str,
        project_path: &str,
        model: Option<&str>,
        day: u32,
        messages: u64,
        input_tokens: u64,
        output_tokens: u64,
    ) -> StatisticsSessionRow {
        StatisticsSessionRow {
            agent: agent.to_string(),
            project_path: project_path.to_string(),
            model: model.map(str::to_string),
            created_at: timestamp(day),
            updated_at: timestamp(day),
            message_count: messages,
            input_tokens,
            output_tokens,
        }
    }

    #[test]
    fn normalizes_unix_windows_and_encoded_project_paths() {
        let rows = vec![
            row("codex", "/Users/maf/My Files/orbit", None, 1, 1, 1, 1),
            row("cursor", r"C:\Users\maf\My Files\orbit", None, 2, 1, 1, 1),
            row("cursor", "-Users-maf-My-Files-orbit", None, 3, 1, 1, 1),
        ];

        let projects = normalized_project_names(&rows);

        assert_eq!(projects, vec!["orbit", "orbit", "orbit"]);
    }

    #[test]
    fn encoded_path_prefers_longest_known_project_suffix() {
        let rows = vec![
            row("codex", "/Users/maf/work/api-server", None, 1, 1, 1, 1),
            row("cursor", "-Users-maf-work-api-server", None, 2, 1, 1, 1),
            row("cursor", "Users-maf-work-api-server", None, 3, 1, 1, 1),
        ];

        let projects = normalized_project_names(&rows);

        assert_eq!(projects, vec!["api-server", "api-server", "api-server"]);
    }

    #[test]
    fn empty_project_path_becomes_unknown_project() {
        let rows = vec![row("codex", "", None, 1, 1, 1, 1)];

        assert_eq!(normalized_project_names(&rows), vec!["Unknown project"]);
    }

    #[test]
    fn agent_dashboard_uses_input_plus_output_tokens() {
        let mut session = row("codex", "/work/orbit", Some("gpt-5-codex"), 10, 12, 100, 25);
        session.updated_at = timestamp(11);

        let dashboard = aggregate_statistics(
            StatisticsMode::Agent,
            StatisticsPeriod::SevenDays,
            timestamp(10),
            &[session],
        );

        let StatisticsDashboard::Agent {
            summary,
            agents,
            models,
            ..
        } = dashboard
        else {
            panic!("expected agent dashboard");
        };
        assert_eq!(summary.sessions, 1);
        assert_eq!(summary.messages, 12);
        assert_eq!(summary.total_tokens, 125);
        assert_eq!(agents[0].tokens, 125);
        assert_eq!(models[0].tokens, 125);
    }

    #[test]
    fn seven_day_agent_timeline_includes_empty_days() {
        let dashboard = aggregate_statistics(
            StatisticsMode::Agent,
            StatisticsPeriod::SevenDays,
            timestamp(10),
            &[row("codex", "/work/orbit", None, 10, 1, 1, 1)],
        );

        let StatisticsDashboard::Agent { timeline, .. } = dashboard else {
            panic!("expected agent dashboard");
        };
        assert_eq!(timeline.len(), 7);
        assert!(timeline[..6].iter().all(|bucket| bucket.values.is_empty()));
        assert_eq!(timeline[6].values[0].value, 1);
    }

    #[test]
    fn ninety_day_timeline_keeps_sessions_at_the_cutoff() {
        let now = Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap();
        let cutoff = Utc.with_ymd_and_hms(2026, 3, 13, 0, 0, 0).unwrap();
        let mut session = row("codex", "/work/orbit", None, 10, 1, 1, 1);
        session.created_at = cutoff;

        let dashboard = aggregate_statistics(
            StatisticsMode::Agent,
            StatisticsPeriod::NinetyDays,
            now,
            &[session],
        );

        let StatisticsDashboard::Agent { timeline, .. } = dashboard else {
            panic!("expected agent dashboard");
        };
        assert_eq!(timeline.len(), 13);
        assert_eq!(timeline[0].values[0].value, 1);
    }

    #[test]
    fn project_dashboard_merges_normal_and_encoded_paths() {
        let rows = vec![
            row("codex", "/Users/maf/work/api-server", None, 8, 10, 100, 20),
            row("cursor", "-Users-maf-work-api-server", None, 9, 5, 30, 10),
        ];

        let dashboard = aggregate_statistics(
            StatisticsMode::Project,
            StatisticsPeriod::SevenDays,
            timestamp(10),
            &rows,
        );

        let StatisticsDashboard::Project { projects, .. } = dashboard else {
            panic!("expected project dashboard");
        };
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project, "api-server");
        assert_eq!(projects[0].sessions, 2);
        assert_eq!(projects[0].tokens, 160);
        assert_eq!(projects[0].agent_count, 2);
    }
}

pub mod accounts_table;
pub mod brokerage_table;
pub mod journal_table;
pub mod notebook_folders_table;
pub mod notebook_images;
pub mod notebook_table;
pub mod playbook_table;
pub mod position_calculator_history_table;
pub mod position_calculator_plans_table;
pub mod position_calculator_rule_table;
pub mod tags_table;
pub mod trading_principle_table;
pub mod user_agents_table;
pub mod user_prompts_table;
pub mod users_table;

// The schema itself now lives in versioned SQL migrations under
// `backend/migrations/` (applied by `super::pg::migrate`). These modules hold
// only the typed query functions for each table.

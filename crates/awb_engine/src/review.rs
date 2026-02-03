use awb_domain::session::*;
use awb_domain::types::*;
use awb_domain::warnings::Warning;

#[derive(Debug, Clone)]
pub enum ReviewState {
    Idle,
    LoadingList,
    FetchingPage { index: usize },
    ApplyingRules { index: usize },
    AwaitingDecision { plan: Box<EditPlan> },
    Saving { index: usize },
    Paused { index: usize },
    Completed { stats: SessionStats },
    Error { error: String, index: usize },
}

#[derive(Debug, Clone)]
pub enum ReviewEvent {
    Start,
    ListLoaded(Vec<Title>),
    PageFetched(PageContent),
    RulesApplied(EditPlan),
    UserDecision(EditDecision),
    SaveComplete(EditResult),
    SaveFailed(String),
    Pause,
    Resume,
    Stop,
}

#[derive(Debug, Clone)]
pub enum ReviewSideEffect {
    FetchPage(Title),
    ApplyRules(PageContent),
    PresentForReview(EditPlan),
    ExecuteEdit {
        title: Title,
        new_text: String,
        summary: String,
    },
    PersistSession,
    EmitWarning(Warning),
    ShowComplete(SessionStats),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionStats {
    pub total: usize,
    pub saved: usize,
    pub skipped: usize,
    pub errors: usize,
    pub elapsed_secs: f64,
}

pub struct ReviewStateMachine {
    pub state: ReviewState,
    pub page_list: Vec<Title>,
    pub current_index: usize,
    stats: SessionStats,
}

impl ReviewStateMachine {
    pub fn new() -> Self {
        Self {
            state: ReviewState::Idle,
            page_list: Vec::new(),
            current_index: 0,
            stats: SessionStats {
                total: 0,
                saved: 0,
                skipped: 0,
                errors: 0,
                elapsed_secs: 0.0,
            },
        }
    }

    pub fn transition(&mut self, event: ReviewEvent) -> Vec<ReviewSideEffect> {
        let mut effects = Vec::new();

        match (&self.state, event) {
            (ReviewState::Idle, ReviewEvent::Start) => {
                self.state = ReviewState::LoadingList;
            }
            (ReviewState::LoadingList, ReviewEvent::ListLoaded(list)) => {
                self.stats.total = list.len();
                self.page_list = list;
                self.current_index = 0;
                if let Some(title) = self.page_list.first() {
                    self.state = ReviewState::FetchingPage { index: 0 };
                    effects.push(ReviewSideEffect::FetchPage(title.clone()));
                } else {
                    self.state = ReviewState::Completed {
                        stats: self.stats.clone(),
                    };
                    effects.push(ReviewSideEffect::ShowComplete(self.stats.clone()));
                }
            }
            (ReviewState::FetchingPage { index }, ReviewEvent::PageFetched(page)) => {
                let idx = *index;
                self.state = ReviewState::ApplyingRules { index: idx };
                effects.push(ReviewSideEffect::ApplyRules(page));
            }
            (ReviewState::ApplyingRules { .. }, ReviewEvent::RulesApplied(plan)) => {
                self.state = ReviewState::AwaitingDecision {
                    plan: Box::new(plan.clone()),
                };
                effects.push(ReviewSideEffect::PresentForReview(plan));
            }
            (ReviewState::AwaitingDecision { plan }, ReviewEvent::UserDecision(decision)) => {
                match decision {
                    EditDecision::Save => {
                        let idx = self.current_index;
                        effects.push(ReviewSideEffect::ExecuteEdit {
                            title: plan.page.title.clone(),
                            new_text: plan.new_wikitext.clone(),
                            summary: plan.summary.clone(),
                        });
                        self.state = ReviewState::Saving { index: idx };
                    }
                    EditDecision::Skip => {
                        self.stats.skipped += 1;
                        self.advance(&mut effects);
                    }
                    EditDecision::Pause => {
                        self.state = ReviewState::Paused {
                            index: self.current_index,
                        };
                        effects.push(ReviewSideEffect::PersistSession);
                    }
                    EditDecision::OpenInBrowser => {
                        // UI handles this; stay in same state
                    }
                    EditDecision::ManualEdit(_) => {
                        self.stats.skipped += 1;
                        self.advance(&mut effects);
                    }
                }
            }
            (ReviewState::Saving { .. }, ReviewEvent::SaveComplete(_result)) => {
                self.stats.saved += 1;
                self.advance(&mut effects);
            }
            (ReviewState::Saving { index }, ReviewEvent::SaveFailed(err)) => {
                self.stats.errors += 1;
                self.state = ReviewState::Error {
                    error: err,
                    index: *index,
                };
            }
            (ReviewState::Error { index: _, .. }, ReviewEvent::Resume) => {
                self.advance(&mut effects);
            }
            (ReviewState::Paused { .. }, ReviewEvent::Resume) => {
                self.advance(&mut effects);
            }
            (_, ReviewEvent::Stop) => {
                self.state = ReviewState::Completed {
                    stats: self.stats.clone(),
                };
                effects.push(ReviewSideEffect::PersistSession);
                effects.push(ReviewSideEffect::ShowComplete(self.stats.clone()));
            }
            (_state, _event) => {
                tracing::warn!("Unexpected state machine transition ignored");
            }
        }
        effects
    }

    fn advance(&mut self, effects: &mut Vec<ReviewSideEffect>) {
        self.current_index += 1;
        if self.current_index < self.page_list.len() {
            let title = self.page_list[self.current_index].clone();
            self.state = ReviewState::FetchingPage {
                index: self.current_index,
            };
            effects.push(ReviewSideEffect::FetchPage(title));
        } else {
            self.state = ReviewState::Completed {
                stats: self.stats.clone(),
            };
            effects.push(ReviewSideEffect::PersistSession);
            effects.push(ReviewSideEffect::ShowComplete(self.stats.clone()));
        }
    }

    pub fn state(&self) -> &ReviewState {
        &self.state
    }
}

impl Default for ReviewStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_title(name: &str) -> Title {
        Title::new(Namespace::MAIN, name)
    }

    fn create_test_page(title: Title) -> PageContent {
        PageContent {
            page_id: PageId(1),
            title,
            revision: RevisionId(100),
            timestamp: chrono::Utc::now(),
            wikitext: "test content".to_string(),
            size_bytes: 100,
            is_redirect: false,
            protection: ProtectionInfo::default(),
            properties: PageProperties::default(),
        }
    }

    fn create_test_plan(page: PageContent) -> EditPlan {
        EditPlan {
            page,
            new_wikitext: "modified content".to_string(),
            rules_applied: vec![],
            fixes_applied: vec![],
            diff_ops: vec![],
            summary: "test edit".to_string(),
            warnings: vec![],
            is_cosmetic_only: false,
        }
    }

    #[test]
    fn test_review_state_machine_new() {
        let machine = ReviewStateMachine::new();
        assert!(matches!(machine.state, ReviewState::Idle));
        assert_eq!(machine.page_list.len(), 0);
        assert_eq!(machine.current_index, 0);
    }

    #[test]
    fn test_transition_start() {
        let mut machine = ReviewStateMachine::new();
        let effects = machine.transition(ReviewEvent::Start);

        assert!(matches!(machine.state, ReviewState::LoadingList));
        assert_eq!(effects.len(), 0);
    }

    #[test]
    fn test_transition_list_loaded_empty() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let effects = machine.transition(ReviewEvent::ListLoaded(vec![]));

        assert!(matches!(machine.state, ReviewState::Completed { .. }));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], ReviewSideEffect::ShowComplete(_)));
    }

    #[test]
    fn test_transition_list_loaded_with_pages() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let titles = vec![create_test_title("Page1"), create_test_title("Page2")];
        let effects = machine.transition(ReviewEvent::ListLoaded(titles.clone()));

        assert!(matches!(
            machine.state,
            ReviewState::FetchingPage { index: 0 }
        ));
        assert_eq!(machine.stats.total, 2);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], ReviewSideEffect::FetchPage(_)));
    }

    #[test]
    fn test_transition_page_fetched() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let title = create_test_title("Test");
        machine.transition(ReviewEvent::ListLoaded(vec![title.clone()]));

        let page = create_test_page(title);
        let effects = machine.transition(ReviewEvent::PageFetched(page.clone()));

        assert!(matches!(
            machine.state,
            ReviewState::ApplyingRules { index: 0 }
        ));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], ReviewSideEffect::ApplyRules(_)));
    }

    #[test]
    fn test_transition_rules_applied() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let title = create_test_title("Test");
        machine.transition(ReviewEvent::ListLoaded(vec![title.clone()]));

        let page = create_test_page(title);
        machine.transition(ReviewEvent::PageFetched(page.clone()));

        let plan = create_test_plan(page);
        let effects = machine.transition(ReviewEvent::RulesApplied(plan.clone()));

        assert!(matches!(
            machine.state,
            ReviewState::AwaitingDecision { .. }
        ));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], ReviewSideEffect::PresentForReview(_)));
    }

    #[test]
    fn test_transition_user_decision_save() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let title = create_test_title("Test");
        machine.transition(ReviewEvent::ListLoaded(vec![title.clone()]));

        let page = create_test_page(title);
        machine.transition(ReviewEvent::PageFetched(page.clone()));

        let plan = create_test_plan(page);
        machine.transition(ReviewEvent::RulesApplied(plan));

        let effects = machine.transition(ReviewEvent::UserDecision(EditDecision::Save));

        assert!(matches!(machine.state, ReviewState::Saving { index: 0 }));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], ReviewSideEffect::ExecuteEdit { .. }));
    }

    #[test]
    fn test_transition_user_decision_skip() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let titles = vec![create_test_title("Page1"), create_test_title("Page2")];
        machine.transition(ReviewEvent::ListLoaded(titles.clone()));

        let page = create_test_page(titles[0].clone());
        machine.transition(ReviewEvent::PageFetched(page.clone()));

        let plan = create_test_plan(page);
        machine.transition(ReviewEvent::RulesApplied(plan));

        let effects = machine.transition(ReviewEvent::UserDecision(EditDecision::Skip));

        assert_eq!(machine.stats.skipped, 1);
        // Should advance to next page
        assert!(matches!(
            machine.state,
            ReviewState::FetchingPage { index: 1 }
        ));
    }

    #[test]
    fn test_transition_user_decision_pause() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let title = create_test_title("Test");
        machine.transition(ReviewEvent::ListLoaded(vec![title.clone()]));

        let page = create_test_page(title);
        machine.transition(ReviewEvent::PageFetched(page.clone()));

        let plan = create_test_plan(page);
        machine.transition(ReviewEvent::RulesApplied(plan));

        let effects = machine.transition(ReviewEvent::UserDecision(EditDecision::Pause));

        assert!(matches!(machine.state, ReviewState::Paused { index: 0 }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, ReviewSideEffect::PersistSession))
        );
    }

    #[test]
    fn test_transition_save_complete() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let title = create_test_title("Test");
        machine.transition(ReviewEvent::ListLoaded(vec![title.clone()]));

        let page = create_test_page(title);
        machine.transition(ReviewEvent::PageFetched(page.clone()));

        let plan = create_test_plan(page);
        machine.transition(ReviewEvent::RulesApplied(plan));
        machine.transition(ReviewEvent::UserDecision(EditDecision::Save));

        let result = EditResult {
            page_id: PageId(1),
            new_revision: Some(RevisionId(101)),
            outcome: EditOutcome::Saved {
                revision: RevisionId(101),
            },
            timestamp: chrono::Utc::now(),
        };

        let effects = machine.transition(ReviewEvent::SaveComplete(result));

        assert_eq!(machine.stats.saved, 1);
        // Should complete since only one page
        assert!(matches!(machine.state, ReviewState::Completed { .. }));
    }

    #[test]
    fn test_transition_save_failed() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let title = create_test_title("Test");
        machine.transition(ReviewEvent::ListLoaded(vec![title.clone()]));

        let page = create_test_page(title);
        machine.transition(ReviewEvent::PageFetched(page.clone()));

        let plan = create_test_plan(page);
        machine.transition(ReviewEvent::RulesApplied(plan));
        machine.transition(ReviewEvent::UserDecision(EditDecision::Save));

        let effects = machine.transition(ReviewEvent::SaveFailed("Network error".to_string()));

        assert_eq!(machine.stats.errors, 1);
        assert!(matches!(machine.state, ReviewState::Error { .. }));
    }

    #[test]
    fn test_transition_resume_from_paused() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let titles = vec![create_test_title("Page1"), create_test_title("Page2")];
        machine.transition(ReviewEvent::ListLoaded(titles.clone()));

        let page = create_test_page(titles[0].clone());
        machine.transition(ReviewEvent::PageFetched(page.clone()));

        let plan = create_test_plan(page);
        machine.transition(ReviewEvent::RulesApplied(plan));
        machine.transition(ReviewEvent::UserDecision(EditDecision::Pause));

        let effects = machine.transition(ReviewEvent::Resume);

        // Should advance to next page
        assert!(matches!(
            machine.state,
            ReviewState::FetchingPage { index: 1 }
        ));
    }

    #[test]
    fn test_transition_stop() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let title = create_test_title("Test");
        machine.transition(ReviewEvent::ListLoaded(vec![title]));

        let effects = machine.transition(ReviewEvent::Stop);

        assert!(matches!(machine.state, ReviewState::Completed { .. }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, ReviewSideEffect::PersistSession))
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, ReviewSideEffect::ShowComplete(_)))
        );
    }

    #[test]
    fn test_stats_tracking() {
        let mut machine = ReviewStateMachine::new();
        assert_eq!(machine.stats.total, 0);
        assert_eq!(machine.stats.saved, 0);
        assert_eq!(machine.stats.skipped, 0);
        assert_eq!(machine.stats.errors, 0);

        machine.transition(ReviewEvent::Start);
        machine.transition(ReviewEvent::ListLoaded(vec![create_test_title("P1")]));

        assert_eq!(machine.stats.total, 1);
    }

    #[test]
    fn test_multiple_pages_workflow() {
        let mut machine = ReviewStateMachine::new();
        machine.transition(ReviewEvent::Start);

        let titles = vec![
            create_test_title("Page1"),
            create_test_title("Page2"),
            create_test_title("Page3"),
        ];
        machine.transition(ReviewEvent::ListLoaded(titles.clone()));

        assert_eq!(machine.stats.total, 3);
        assert_eq!(machine.current_index, 0);

        // Process first page - save
        let page1 = create_test_page(titles[0].clone());
        machine.transition(ReviewEvent::PageFetched(page1.clone()));
        machine.transition(ReviewEvent::RulesApplied(create_test_plan(page1)));
        machine.transition(ReviewEvent::UserDecision(EditDecision::Save));
        machine.transition(ReviewEvent::SaveComplete(EditResult {
            page_id: PageId(1),
            new_revision: Some(RevisionId(101)),
            outcome: EditOutcome::Saved {
                revision: RevisionId(101),
            },
            timestamp: chrono::Utc::now(),
        }));

        assert_eq!(machine.stats.saved, 1);
        assert_eq!(machine.current_index, 1);

        // Process second page - skip
        let page2 = create_test_page(titles[1].clone());
        machine.transition(ReviewEvent::PageFetched(page2.clone()));
        machine.transition(ReviewEvent::RulesApplied(create_test_plan(page2)));
        machine.transition(ReviewEvent::UserDecision(EditDecision::Skip));

        assert_eq!(machine.stats.skipped, 1);
        assert_eq!(machine.current_index, 2);

        // Process third page - save
        let page3 = create_test_page(titles[2].clone());
        machine.transition(ReviewEvent::PageFetched(page3.clone()));
        machine.transition(ReviewEvent::RulesApplied(create_test_plan(page3)));
        machine.transition(ReviewEvent::UserDecision(EditDecision::Save));
        machine.transition(ReviewEvent::SaveComplete(EditResult {
            page_id: PageId(1),
            new_revision: Some(RevisionId(102)),
            outcome: EditOutcome::Saved {
                revision: RevisionId(102),
            },
            timestamp: chrono::Utc::now(),
        }));

        assert_eq!(machine.stats.saved, 2);
        assert!(matches!(machine.state, ReviewState::Completed { .. }));
    }
}

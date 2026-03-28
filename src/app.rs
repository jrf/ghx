use crate::gh;
use crate::theme::{self, Theme};
use crate::ui::lists_view::ListsView;
use crate::ui::notif_list::NotifList;
use crate::ui::repo_detail::RepoDetailView;
use crate::ui::repo_list::RepoList;
use crate::ui::search::SearchView;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    Home,
    RepoDetail,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Repos,
    Lists,
    Search,
    Notifications,
}

pub struct App {
    pub screen: Screen,
    pub tab: Tab,
    pub repo_list: RepoList,
    pub repo_detail: Option<RepoDetailView>,
    pub lists_view: ListsView,
    pub notif_list: NotifList,
    pub search: SearchView,
    pub selected_repo: Option<String>,
    pub context_repo: Option<String>,
    pub should_quit: bool,
    pub show_help: bool,
    pub tick: usize,
    // Theme picker
    pub show_theme_picker: bool,
    pub themes: Vec<(String, Theme)>,
    pub theme_index: usize,
    pub original_theme_index: usize,
}

impl App {
    pub fn new(context_repo: Option<String>) -> Self {
        let themes = theme::load_all_themes();
        let configured = theme::configured_theme_name();
        let theme_index = themes
            .iter()
            .position(|(n, _)| n == &configured)
            .unwrap_or(0);

        Self {
            screen: Screen::Home,
            tab: Tab::Repos,
            repo_list: RepoList::new(),
            repo_detail: None,
            lists_view: ListsView::new(),
            notif_list: NotifList::new(),
            search: SearchView::new(),
            selected_repo: None,
            context_repo,
            should_quit: false,
            show_help: false,
            tick: 0,
            show_theme_picker: false,
            themes,
            theme_index,
            original_theme_index: 0,
        }
    }

    pub fn init(&mut self) {
        self.repo_list.load_orgs();
        self.repo_list.load();

        if let Some(ref repo) = self.context_repo {
            let name = repo.clone();
            self.selected_repo = Some(name.clone());
            self.repo_detail = Some(RepoDetailView::new(name));
            self.screen = Screen::RepoDetail;
        }
    }

    pub fn open_theme_picker(&mut self) {
        self.original_theme_index = self.theme_index;
        self.show_theme_picker = true;
    }

    pub fn theme_picker_select(&mut self, index: usize) {
        if index < self.themes.len() {
            self.theme_index = index;
            theme::set_theme(self.themes[index].1.clone());
        }
    }

    pub fn theme_picker_confirm(&mut self) {
        self.show_theme_picker = false;
        let name = &self.themes[self.theme_index].0;
        theme::save_config_theme(name);
    }

    pub fn theme_picker_cancel(&mut self) {
        self.theme_index = self.original_theme_index;
        theme::set_theme(self.themes[self.theme_index].1.clone());
        self.show_theme_picker = false;
    }

    pub fn next_tab(&mut self) {
        let total = self.repo_list.total_sources();
        let current = self.repo_list.active_source_index();

        match self.tab {
            Tab::Repos => {
                if current + 1 < total {
                    self.repo_list.set_source_by_index(current + 1);
                    self.repo_list.load();
                } else {
                    self.tab = Tab::Lists;
                }
            }
            Tab::Lists => self.tab = Tab::Search,
            Tab::Search => self.tab = Tab::Notifications,
            Tab::Notifications => {
                self.tab = Tab::Repos;
                self.repo_list.set_source_by_index(0);
                self.repo_list.load();
            }
        }
    }

    pub fn prev_tab(&mut self) {
        let total = self.repo_list.total_sources();
        let current = self.repo_list.active_source_index();

        match self.tab {
            Tab::Repos => {
                if current > 0 {
                    self.repo_list.set_source_by_index(current - 1);
                    self.repo_list.load();
                } else {
                    self.tab = Tab::Notifications;
                }
            }
            Tab::Lists => {
                self.tab = Tab::Repos;
                self.repo_list.set_source_by_index(total - 1);
                self.repo_list.load();
            }
            Tab::Search => self.tab = Tab::Lists,
            Tab::Notifications => self.tab = Tab::Search,
        }
    }

    pub fn on_enter(&mut self) {
        match self.screen {
            Screen::Home if self.tab == Tab::Repos => {
                if let Some(repo) = self.repo_list.selected_repo() {
                    self.enter_repo(repo.full_name.clone());
                }
            }
            Screen::Home if self.tab == Tab::Lists => {
                if self.lists_view.is_browsing_repos() {
                    if let Some(repo) = self.lists_view.selected_repo() {
                        self.enter_repo(repo.full_name.clone());
                    }
                } else {
                    self.lists_view.enter();
                }
            }
            Screen::Home if self.tab == Tab::Search => {
                if let Some(repo) = self.search.selected_repo() {
                    self.enter_repo(repo.full_name.clone());
                }
            }
            _ => {}
        }
    }

    fn enter_repo(&mut self, name: String) {
        self.selected_repo = Some(name.clone());
        self.repo_detail = Some(RepoDetailView::new(name));
        self.screen = Screen::RepoDetail;
    }

    pub fn go_back(&mut self) {
        match self.screen {
            Screen::RepoDetail => {
                self.screen = Screen::Home;
                self.repo_detail = None;
                self.selected_repo = None;
            }
            _ => {}
        }
    }

    pub fn on_open(&self) {
        match self.screen {
            Screen::Home if self.tab == Tab::Repos => {
                if let Some(repo) = self.repo_list.selected_repo() {
                    gh::open_repo(&repo.full_name);
                }
            }
            Screen::Home if self.tab == Tab::Lists => {
                if let Some(repo) = self.lists_view.selected_repo() {
                    gh::open_repo(&repo.full_name);
                }
            }
            Screen::Home if self.tab == Tab::Search => {
                if let Some(repo) = self.search.selected_repo() {
                    gh::open_repo(&repo.full_name);
                }
            }
            Screen::RepoDetail => {
                if let Some(ref detail) = self.repo_detail {
                    if let Some(ref name) = self.selected_repo {
                        match detail.tab {
                            crate::ui::repo_detail::RepoTab::Issues => {
                                if let Some(number) = detail.selected_issue_number() {
                                    gh::open_issue(name, number);
                                } else {
                                    gh::open_repo(name);
                                }
                            }
                            crate::ui::repo_detail::RepoTab::PullRequests => {
                                if let Some(number) = detail.selected_pr_number() {
                                    gh::open_pr(name, number);
                                } else {
                                    gh::open_repo(name);
                                }
                            }
                            _ => gh::open_repo(name),
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

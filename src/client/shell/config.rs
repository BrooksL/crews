use super::*;

pub(super) fn merged_config_diagnostic(
    local: Option<&str>,
    endpoint: Option<&str>,
) -> Option<String> {
    match (local, endpoint) {
        (Some(local), Some(endpoint)) if local == endpoint => {
            Some(format!("client + endpoint: {local}"))
        }
        (Some(local), Some(endpoint)) => Some(format!("client: {local}\nendpoint: {endpoint}")),
        (Some(local), None) => Some(local.to_owned()),
        (None, Some(endpoint)) => Some(endpoint.to_owned()),
        (None, None) => None,
    }
}

impl ClientShellState {
    pub(super) fn set_local_config_diagnostic(&mut self, diagnostic: Option<String>) {
        self.local_config_diagnostic = diagnostic;
        self.config_diagnostic = merged_config_diagnostic(
            self.local_config_diagnostic.as_deref(),
            self.snapshot
                .as_deref()
                .and_then(|snapshot| snapshot.config_diagnostic.as_deref()),
        );
    }

    pub(super) fn persist_chrome_preferences(&mut self, outcome: &mut ClientShellInput) {
        let Some(path) = self.config.preferences_path.as_deref() else {
            return;
        };
        let mut collapsed_groups = self.collapsed_groups.iter().cloned().collect::<Vec<_>>();
        collapsed_groups.sort();
        let mut remote_collapsed_groups = self
            .remote_collapsed_groups
            .iter()
            .filter_map(|(endpoint_id, groups)| {
                let ClientEndpointId::Ssh(profile_id) = endpoint_id else {
                    return None;
                };
                let mut collapsed_groups = groups.iter().cloned().collect::<Vec<_>>();
                collapsed_groups.sort();
                (!collapsed_groups.is_empty()).then(|| preferences::ClientRemoteCollapsedGroups {
                    profile_id: profile_id.to_string(),
                    collapsed_groups,
                })
            })
            .collect::<Vec<_>>();
        remote_collapsed_groups.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        let preferences = preferences::ClientChromePreferences {
            sidebar_width: self.sidebar_width_manual.then_some(self.sidebar_width),
            sidebar_section_split: self
                .sidebar_section_split_manual
                .then_some(self.sidebar_section_split),
            sidebar_collapsed: self
                .sidebar_collapsed_manual
                .then_some(self.sidebar_collapsed),
            sidebar_spaces_visible: (!self.sidebar_spaces_visible).then_some(false),
            sidebar_agents_visible: (!self.sidebar_agents_visible).then_some(false),
            agent_panel_sort: self
                .agent_panel_sort_manual
                .then_some(self.config.agent_panel_sort),
            collapsed_groups,
            remote_collapsed_groups,
        };
        if let Err(error) = preferences::store(path, preferences) {
            self.set_endpoint_error(error);
            outcome.repaint = true;
        }
    }

    /// Show or hide one section of the app-style sidebar and remember it.
    pub(super) fn toggle_sidebar_section(
        &mut self,
        section: SidebarSection,
        outcome: &mut ClientShellInput,
    ) {
        match section {
            SidebarSection::Spaces => {
                self.sidebar_spaces_visible = !self.sidebar_spaces_visible;
                self.workspace_scroll = 0;
            }
            SidebarSection::Agents => {
                self.sidebar_agents_visible = !self.sidebar_agents_visible;
                self.agent_scroll = 0;
            }
        }
        outcome.repaint = true;
        self.persist_chrome_preferences(outcome);
    }

    pub(crate) fn reload_client_config(&mut self) {
        match crate::config::load_live_config() {
            Ok(loaded) => {
                let agent_panel_sort = self.config.agent_panel_sort;
                let mut diagnostics = self.config.apply_live_config(
                    &loaded.config,
                    &loaded.diagnostics,
                    &loaded.invalid_sections,
                );
                diagnostics.extend(app_style_diagnostic(&loaded.config));
                if let Some(appearance) = self.host_appearance {
                    self.config.palette = crate::app::client_palette_for_appearance(
                        &self.config.theme_runtime,
                        appearance,
                    );
                }
                if !self.sidebar_width_manual {
                    self.sidebar_width = self.config.sidebar_width;
                }
                if self.agent_panel_sort_manual {
                    self.config.agent_panel_sort = agent_panel_sort;
                }
                self.set_local_config_diagnostic(self.config.local_config_diagnostic(&diagnostics));
                if let Some(snapshot) = self.snapshot.as_deref() {
                    let profile = snapshot.server_keybindings_toml.clone();
                    let commands = snapshot.commands.clone();
                    if let Err(err) = self
                        .config
                        .apply_snapshot_keybindings(profile.as_deref(), &commands)
                    {
                        self.set_endpoint_error(err);
                    }
                }
            }
            Err(diagnostics) => {
                self.set_local_config_diagnostic(self.config.local_config_diagnostic(&diagnostics));
            }
        }
        self.reconcile_input_source();
    }
}

/// Columns of panel fill on each side of app content.
pub(super) const APP_PANEL_PADDING: u16 = 2;
/// Smallest panel that app style lays out; anything smaller keeps classic.
const APP_MIN_PANEL_WIDTH: u16 = 20;
const APP_MIN_ROWS: u16 = 8;

/// App-style window regions, or None when the window is too small for them.
///
/// The sidebar's last column is the left gutter (column 0 when the sidebar is
/// hidden), column `cols - 1` is the right gutter, and the last row is the
/// status bar. The panel's cap rows are its first and last rows. Row 1 is the
/// header, rows 2 and `rows - 3` are padding, and pane content fills the rows
/// between them, inset two columns from each side.
fn app_layout(cols: u16, rows: u16, sidebar_width: u16) -> Option<ClientShellLayout> {
    let panel_x = sidebar_width.max(1);
    let panel_width = cols.checked_sub(panel_x + 1)?;
    if rows < APP_MIN_ROWS || panel_width < APP_MIN_PANEL_WIDTH {
        return None;
    }
    let panel = Rect::new(panel_x, 0, panel_width, rows - 1);
    Some(ClientShellLayout {
        sidebar: Rect::new(0, 0, sidebar_width, rows - 1),
        tab_bar: Rect::new(panel_x, 1, panel_width, 1),
        mobile_header: Rect::default(),
        pane_surface: Rect::new(
            panel_x + APP_PANEL_PADDING,
            3,
            panel_width - 2 * APP_PANEL_PADDING,
            rows - 6,
        ),
        panel,
        status_bar: Rect::new(1, rows - 1, cols - 2, 1),
        panel_padding_rows: [2, rows - 3],
    })
}

/// Sidebar width for app style when the user kept the default: the polished
/// sidebar is laid out for 36 columns. Explicit widths and classic are untouched.
fn app_style_sidebar_width(config: &Config) -> u16 {
    let ui = &config.ui;
    let default_width = Config::default().ui.sidebar_width;
    if ui.style == crate::config::UiStyleConfig::App && ui.sidebar_width == default_width {
        APP_STYLE_SIDEBAR_WIDTH.min(ui.sidebar_max_width.max(ui.sidebar_min_width))
    } else {
        ui.sidebar_width
    }
}

pub(super) const APP_STYLE_SIDEBAR_WIDTH: u16 = 36;

fn clamp_dim(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Warn when `ui.style = "app"` can't apply because a selected theme has no
/// polished-shell colors (light and ANSI-only themes keep the classic look).
pub(crate) fn app_style_diagnostic(config: &Config) -> Option<String> {
    if config.ui.style != crate::config::UiStyleConfig::App {
        return None;
    }
    let runtime = crate::app::client_theme_runtime_from_config(config);
    let names = if runtime.auto_switch {
        vec![runtime.dark_name.clone(), runtime.light_name.clone()]
    } else {
        vec![runtime.manual_name.clone()]
    };
    let classic = names
        .into_iter()
        .filter(|name| {
            crate::app::client_palette_for_theme(&runtime, name)
                .app
                .is_none()
        })
        .collect::<Vec<_>>();
    (!classic.is_empty()).then(|| {
        format!(
            "ui.style = \"app\" needs a dark theme; {} keeps the classic look",
            classic
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(" and ")
        )
    })
}

impl ClientShellConfig {
    /// Polished-shell colors when app style is on and the current theme supports it.
    pub(super) fn app_tokens(&self) -> Option<&crate::app::app_tokens::AppTokens> {
        (self.style == crate::config::UiStyleConfig::App)
            .then_some(self.palette.app.as_ref())
            .flatten()
    }
}

impl ClientShellConfig {
    pub(crate) fn from_config(config: &Config) -> Self {
        let theme_runtime = crate::app::client_theme_runtime_from_config(config);
        Self {
            style: config.ui.style,
            dim_unfocused: clamp_dim(config.ui.dim_unfocused),
            corners: config.ui.corners,
            sidebar_width: app_style_sidebar_width(config),
            sidebar_min_width: config.ui.sidebar_min_width,
            sidebar_max_width: config.ui.sidebar_max_width,
            sidebar_start_collapsed: config.ui.sidebar_start_collapsed,
            sidebar_collapsed_mode: config.ui.sidebar_collapsed_mode,
            mobile_width_threshold: config.ui.mobile_width_threshold,
            tab_bar_position: config.ui.tab_bar_position,
            hide_tab_bar_when_single_tab: config.ui.hide_tab_bar_when_single_tab,
            spaces: config.ui.sidebar.spaces.clone(),
            agents: config.ui.sidebar.agents.clone(),
            agent_panel_sort: config.ui.agent_panel_sort,
            status_indicators: config.ui.status_indicators,
            sound_enabled: config.ui.sound.enabled,
            toast_delivery: config.ui.toast.delivery,
            toast_delay_seconds: config.ui.toast.delay_seconds,
            toast_position: config.ui.toast.herdr.position,
            toast_keep_needs_attention: config.ui.toast.herdr.keep_needs_attention,
            copy_on_select: config.ui.copy_on_select,
            clipboard_toast_enabled: config.ui.toast.clipboard.enabled,
            clipboard_toast_position: config.ui.toast.clipboard.position,
            theme_name: theme_runtime.manual_name.clone(),
            theme_runtime,
            palette: crate::app::client_palette_from_config(config),
            keybinds: config
                .live_keybinds_with_diagnostics()
                .map(|(keybinds, _diagnostics)| keybinds)
                .unwrap_or_else(|_diagnostics| LiveKeybindConfig {
                    prefix: config.prefix_keys(),
                    keybinds: config.keybinds(),
                }),
            local_keys: config.keys.clone(),
            keybinding_source: ClientShellKeybindingSource::Local,
            prompt_new_tab_name: config.ui.prompt_new_tab_name,
            prompt_new_workspace_name: config.ui.prompt_new_workspace_name,
            confirm_close: config.ui.confirm_close,
            mouse_capture: config.ui.mouse_capture,
            mouse_scroll_lines: config.ui.mouse_scroll_lines(),
            right_click_passthrough_modifiers: config.ui.right_click_passthrough_modifiers(),
            redraw_on_focus_gained: config.ui.redraw_on_focus_gained,
            switch_ascii_input_source_in_prefix: config
                .experimental
                .switch_ascii_input_source_in_prefix,
            local_config_path: crate::config::config_path(),
            preferences_path: None,
            preferences: preferences::ClientChromePreferences::default(),
            startup_config_diagnostic: None,
            startup_onboarding: false,
        }
    }

    pub(crate) fn with_startup_config_diagnostic(mut self, diagnostic: Option<String>) -> Self {
        self.startup_config_diagnostic = diagnostic;
        self
    }

    pub(crate) fn with_startup_onboarding(mut self, show: bool) -> Self {
        self.startup_onboarding = show;
        self
    }

    pub(crate) fn with_keybinding_source(mut self, source: ClientShellKeybindingSource) -> Self {
        self.keybinding_source = source;
        self.keybinds.keybinds.custom_commands.clear();
        self
    }

    pub(crate) fn uses_endpoint_keybindings(&self) -> bool {
        self.keybinding_source == ClientShellKeybindingSource::Endpoint
    }

    pub(super) fn local_config_diagnostic(&self, diagnostics: &[String]) -> Option<String> {
        if self.uses_endpoint_keybindings() {
            crate::config::config_diagnostic_summary_without_keybindings(diagnostics)
        } else {
            crate::config::config_diagnostic_summary(diagnostics)
        }
    }

    pub(crate) fn with_local_endpoint(self, socket_path: &std::path::Path) -> Self {
        self.with_preferences_path(preferences::path_for_local_endpoint(socket_path))
    }

    pub(crate) fn with_process_endpoint_preferences(self, socket_path: &std::path::Path) -> Self {
        self.with_endpoint_preferences(
            socket_path,
            std::env::var(crate::remote::REMOTE_PREFERENCES_ENV_VAR)
                .ok()
                .as_deref(),
        )
    }

    pub(crate) fn with_endpoint_preferences(
        self,
        socket_path: &std::path::Path,
        remote_identity: Option<&str>,
    ) -> Self {
        let identity = remote_identity
            .and_then(|value| serde_json::from_str::<(String, String)>(value).ok())
            .filter(|(target, session)| !target.is_empty() && !session.is_empty());
        match identity {
            Some((target, session)) => {
                self.with_preferences_path(preferences::path_for_remote_endpoint(&target, &session))
            }
            None => self.with_local_endpoint(socket_path),
        }
    }

    pub(super) fn with_preferences_path(mut self, path: std::path::PathBuf) -> Self {
        self.preferences = preferences::load(&path).unwrap_or_default();
        self.preferences_path = Some(path);
        self
    }

    pub(super) fn apply_snapshot_keybindings(
        &mut self,
        profile: Option<&str>,
        commands: &[crate::protocol::ClientShellCommand],
    ) -> Result<(), String> {
        let mut keybinds = match self.keybinding_source {
            ClientShellKeybindingSource::Endpoint => crate::config::keybindings_from_profile_toml(
                profile.ok_or("endpoint did not publish its keybindings")?,
            )?,
            ClientShellKeybindingSource::RemoteLocal => return Ok(()),
            ClientShellKeybindingSource::Local => {
                let mut config = crate::config::Config {
                    keys: self.local_keys.clone(),
                    ..Default::default()
                };
                config.keys.command = commands
                    .iter()
                    .filter_map(|command| {
                        let action_type = match command.action {
                            crate::protocol::ClientShellCommandAction::Shell => {
                                crate::config::CommandKeybindType::Shell
                            }
                            crate::protocol::ClientShellCommandAction::Pane => {
                                crate::config::CommandKeybindType::Pane
                            }
                            crate::protocol::ClientShellCommandAction::Popup => {
                                crate::config::CommandKeybindType::Popup
                            }
                            crate::protocol::ClientShellCommandAction::PluginAction => {
                                crate::config::CommandKeybindType::PluginAction
                            }
                            crate::protocol::ClientShellCommandAction::Unknown => return None,
                        };
                        Some(crate::config::CommandKeybindConfig {
                            key: if command.binding_labels.len() == 1 {
                                crate::config::BindingConfig::One(command.binding_labels[0].clone())
                            } else {
                                crate::config::BindingConfig::Many(command.binding_labels.clone())
                            },
                            // The client never executes this field; preserve the opaque endpoint ID
                            // through the shared config collision resolver.
                            command: command.command_id.clone(),
                            action_type,
                            description: command.description.clone(),
                            width: None,
                            height: None,
                        })
                    })
                    .collect();
                config
                    .live_keybinds_with_diagnostics()
                    .map(|(keybinds, _diagnostics)| keybinds)
                    .map_err(|diagnostics| diagnostics.join("; "))?
            }
        };
        if self.keybinding_source == ClientShellKeybindingSource::Endpoint {
            for command in commands {
                let Ok(action) = command.action.try_into() else {
                    continue;
                };
                keybinds
                    .keybinds
                    .custom_commands
                    .push(crate::config::CustomCommandKeybind {
                        bindings: crate::config::ActionKeybinds::from_labels(
                            &command.binding_labels,
                        )?,
                        label: command.binding_label.clone(),
                        command: command.command_id.clone(),
                        action,
                        description: command.description.clone(),
                        width: None,
                        height: None,
                    });
            }
        }
        self.keybinds = keybinds;
        Ok(())
    }

    pub(super) fn apply_live_config(
        &mut self,
        config: &Config,
        load_diagnostics: &[String],
        invalid_sections: &[String],
    ) -> Vec<String> {
        let mut diagnostics = load_diagnostics.to_vec();
        let invalid_section =
            |section: &str| invalid_sections.iter().any(|invalid| invalid == section);

        if !invalid_section("keys")
            && self.keybinding_source != ClientShellKeybindingSource::Endpoint
        {
            match config.live_keybinds_with_diagnostics() {
                Ok((mut keybinds, keybind_diagnostics)) => {
                    self.local_keys = config.keys.clone();
                    if self.keybinding_source == ClientShellKeybindingSource::RemoteLocal {
                        keybinds.keybinds.custom_commands.clear();
                    }
                    self.keybinds = keybinds;
                    diagnostics.extend(keybind_diagnostics);
                }
                Err(keybind_diagnostics) => diagnostics.extend(
                    keybind_diagnostics
                        .into_iter()
                        .map(|diagnostic| format!("{diagnostic}; kept current keybinds")),
                ),
            }
        }

        if !invalid_section("ui") {
            if let Some(diagnostic) = config.invalid_sidebar_bounds_diagnostic() {
                diagnostics.push(format!("{diagnostic}; keeping previous [ui] settings"));
            } else {
                let ui = &config.ui;
                diagnostics.extend(ui.sound.diagnostics());
                self.style = ui.style;
                self.dim_unfocused = clamp_dim(ui.dim_unfocused);
                self.corners = ui.corners;
                self.sidebar_width = app_style_sidebar_width(config);
                self.sidebar_min_width = ui.sidebar_min_width;
                self.sidebar_max_width = ui.sidebar_max_width;
                self.sidebar_collapsed_mode = ui.sidebar_collapsed_mode;
                self.mobile_width_threshold = ui.mobile_width_threshold;
                self.tab_bar_position = ui.tab_bar_position;
                self.hide_tab_bar_when_single_tab = ui.hide_tab_bar_when_single_tab;
                self.spaces = ui.sidebar.spaces.clone();
                self.agents = ui.sidebar.agents.clone();
                self.agent_panel_sort = ui.agent_panel_sort;
                self.status_indicators = ui.status_indicators;
                self.sound_enabled = ui.sound.enabled;
                self.toast_delivery = ui.toast.delivery;
                self.toast_delay_seconds = ui.toast.delay_seconds;
                self.toast_position = ui.toast.herdr.position;
                self.toast_keep_needs_attention = ui.toast.herdr.keep_needs_attention;
                self.copy_on_select = ui.copy_on_select;
                self.clipboard_toast_enabled = ui.toast.clipboard.enabled;
                self.clipboard_toast_position = ui.toast.clipboard.position;
                self.prompt_new_tab_name = ui.prompt_new_tab_name;
                self.prompt_new_workspace_name = ui.prompt_new_workspace_name;
                self.confirm_close = ui.confirm_close;
                self.mouse_capture = ui.mouse_capture;
                self.mouse_scroll_lines = ui.mouse_scroll_lines();
                self.right_click_passthrough_modifiers = ui.right_click_passthrough_modifiers();
                self.redraw_on_focus_gained = ui.redraw_on_focus_gained;
            }
        }

        if !invalid_section("theme") {
            self.theme_runtime = crate::app::client_theme_runtime_from_config(config);
            self.theme_name = self.theme_runtime.manual_name.clone();
            self.palette = crate::app::client_palette_from_config(config);
        }
        if !invalid_section("experimental") {
            self.switch_ascii_input_source_in_prefix =
                config.experimental.switch_ascii_input_source_in_prefix;
        }

        diagnostics
    }

    pub(super) fn layout(
        &self,
        cols: u16,
        rows: u16,
        sidebar_collapsed: bool,
        tab_count: usize,
        sidebar_width: u16,
    ) -> ClientShellLayout {
        if cols <= self.mobile_width_threshold {
            let header_height = rows.min(2);
            return ClientShellLayout {
                sidebar: Rect::default(),
                tab_bar: Rect::default(),
                mobile_header: Rect::new(0, 0, cols, header_height),
                pane_surface: Rect::new(0, header_height, cols, rows.saturating_sub(header_height)),
                panel: Rect::default(),
                status_bar: Rect::default(),
                panel_padding_rows: [0, 0],
            };
        }

        let sidebar_width = if sidebar_collapsed {
            match self.sidebar_collapsed_mode {
                SidebarCollapsedModeConfig::Compact => 4,
                SidebarCollapsedModeConfig::Hidden => 0,
            }
        } else {
            let (min, max) = crate::config::validated_sidebar_bounds(
                self.sidebar_min_width,
                self.sidebar_max_width,
            )
            .unwrap_or((18, 36));
            sidebar_width.clamp(min, max)
        }
        .min(cols.saturating_sub(1));
        if self.app_tokens().is_some() {
            if let Some(layout) = app_layout(cols, rows, sidebar_width) {
                return layout;
            }
        }
        let main = Rect::new(sidebar_width, 0, cols.saturating_sub(sidebar_width), rows);
        let show_tab_bar = rows > 1 && !(self.hide_tab_bar_when_single_tab && tab_count == 1);
        let tab_height = u16::from(show_tab_bar);
        let (tab_bar, pane_surface) = match self.tab_bar_position {
            TabBarPositionConfig::Top => (
                Rect::new(main.x, 0, main.width, tab_height),
                Rect::new(
                    main.x,
                    tab_height,
                    main.width,
                    rows.saturating_sub(tab_height),
                ),
            ),
            TabBarPositionConfig::Bottom => (
                Rect::new(
                    main.x,
                    rows.saturating_sub(tab_height),
                    main.width,
                    tab_height,
                ),
                Rect::new(main.x, 0, main.width, rows.saturating_sub(tab_height)),
            ),
        };

        ClientShellLayout {
            sidebar: Rect::new(0, 0, sidebar_width, rows),
            tab_bar,
            mobile_header: Rect::default(),
            pane_surface,
            panel: Rect::default(),
            status_bar: Rect::default(),
            panel_padding_rows: [0, 0],
        }
    }

    pub(crate) fn initial_surface_size(&self, cols: u16, rows: u16) -> ClientSurfaceSize {
        let sidebar_collapsed = self
            .preferences
            .sidebar_collapsed
            .unwrap_or(self.sidebar_start_collapsed);
        let (min_width, max_width) =
            crate::config::validated_sidebar_bounds(self.sidebar_min_width, self.sidebar_max_width)
                .unwrap_or((18, 36));
        let sidebar_width = self
            .preferences
            .sidebar_width
            .unwrap_or(self.sidebar_width)
            .clamp(min_width, max_width);
        let surface = self
            .layout(cols, rows, sidebar_collapsed, 0, sidebar_width)
            .pane_surface;
        ClientSurfaceSize {
            cols: surface.width.max(1),
            rows: surface.height.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;

    #[test]
    fn live_reload_applies_client_owned_sections() {
        let mut shell = ClientShellConfig::from_config(&Config::default());
        let mut next = Config::default();
        next.ui.sidebar_width = 31;
        next.ui.tab_bar_position = TabBarPositionConfig::Bottom;
        next.ui.agent_panel_sort = crate::config::AgentPanelSortConfig::Priority;
        next.ui.status_indicators = crate::config::StatusIndicatorStyle::Symbols;
        next.ui.sidebar.agents = toml::from_str("rows = [[{ token = 'machine', rules = [{ equals = 'Local', bold = true }] }]]\nrow_gap = 2").unwrap();
        next.keys.prefix = crate::config::BindingConfig::one("ctrl+a");

        let diagnostics = shell.apply_live_config(&next, &[], &[]);

        assert!(diagnostics.is_empty());
        assert_eq!(shell.sidebar_width, 31);
        assert_eq!(shell.tab_bar_position, TabBarPositionConfig::Bottom);
        assert_eq!(
            shell.agent_panel_sort,
            crate::config::AgentPanelSortConfig::Priority
        );
        assert_eq!(
            shell.status_indicators,
            crate::config::StatusIndicatorStyle::Symbols
        );
        assert_eq!(shell.agents.row_gap, 2);
        assert_eq!(
            shell.agents.rows[0][0]
                .style_for_value("Local")
                .unwrap()
                .bold,
            Some(true)
        );
        let previous = shell.agents.clone();
        shell.apply_live_config(
            &Config::default(),
            &[],
            &["ui".to_owned(), "keys".to_owned()],
        );
        assert_eq!(shell.agents, previous);
        assert_eq!(
            shell.keybinds.prefix,
            vec![(KeyCode::Char('a'), KeyModifiers::CONTROL)]
        );
    }

    #[test]
    fn initial_surface_size_uses_persisted_endpoint_chrome() {
        let path = std::env::temp_dir().join(format!(
            "herdr-initial-shell-preferences-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        preferences::store(
            &path,
            preferences::ClientChromePreferences {
                sidebar_width: Some(31),
                sidebar_collapsed: Some(true),
                ..preferences::ClientChromePreferences::default()
            },
        )
        .expect("persist endpoint chrome");
        let config =
            ClientShellConfig::from_config(&Config::default()).with_preferences_path(path.clone());
        let initial = config.initial_surface_size(100, 30);
        let state = ClientShellState::new(config);
        assert_eq!(initial, state.surface_size(100, 30));
        std::fs::remove_file(path).expect("remove endpoint chrome");
    }

    #[test]
    fn live_reload_preserves_invalid_client_owned_sections() {
        let mut initial = Config::default();
        initial.ui.sidebar_width = 29;
        initial.keys.prefix = crate::config::BindingConfig::one("ctrl+x");
        let mut shell = ClientShellConfig::from_config(&initial);

        let mut invalid = Config::default();
        invalid.ui.sidebar_width = 35;
        invalid.keys.prefix = crate::config::BindingConfig::one("ctrl+a");
        let invalid_sections = vec!["ui".to_owned(), "keys".to_owned()];
        shell.apply_live_config(&invalid, &[], &invalid_sections);

        assert_eq!(shell.sidebar_width, 29);
        assert_eq!(
            shell.keybinds.prefix,
            vec![(KeyCode::Char('x'), KeyModifiers::CONTROL)]
        );
    }
}

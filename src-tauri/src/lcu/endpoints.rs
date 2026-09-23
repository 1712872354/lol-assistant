//! LCU HTTP 路径常量（对齐 Go internal/lcu/endpoints.go）。

pub const PATH_BUILD_INFO: &str = "/system/v1/builds";
pub const PATH_CURRENT_SUMMONER: &str = "/lol-summoner/v1/current-summoner";
pub const PATH_GAMEFLOW_PHASE: &str = "/lol-gameflow/v1/gameflow-phase";
pub const PATH_GAMEFLOW_SESSION: &str = "/lol-gameflow/v1/session";
pub const PATH_CHAMP_SELECT_SESSION: &str = "/lol-champ-select/v1/session";
pub const PATH_LOBBY: &str = "/lol-lobby/v2/lobby";
pub const PATH_READY_CHECK: &str = "/lol-matchmaking/v1/ready-check";
pub const PATH_MATCH_HISTORY: &str = "/lol-match-history/v1/products/lol/%s/matches";
pub const PATH_MATCH_HISTORY_SELF: &str =
    "/lol-match-history/v1/products/lol/current-summoner/matches";
pub const PATH_MATCH_GAME_DETAIL: &str = "/lol-match-history/v1/games/%d";
pub const PATH_SUMMONERS_BY_NAME: &str = "/lol-summoner/v1/summoners";
pub const PATH_SUMMONER_BY_PUUID: &str = "/lol-summoner/v1/summoners/by-puuid/%s";
pub const PATH_RANKED_STATS: &str = "/lol-ranked/v1/current-ranked-stats";
pub const PATH_RANKED_STATS_BY_SUMMONER: &str = "/lol-ranked/v1/ranked-stats/%s";
pub const PATH_SUMMONER_BY_ID: &str = "/lol-summoner/v1/summoners/%s";
pub const PATH_LEAGUE_SESSION_TOKEN: &str = "/lol-league-session/v1/league-session-token";
pub const PATH_ENTITLEMENTS_TOKEN: &str = "/entitlements/v1/token";

/// ── LCU 游戏资源表与图标（/lol-game-data/ 前缀已在白名单内）──
pub const PATH_GD_ITEMS: &str = "/lol-game-data/assets/v1/items.json";
pub const PATH_GD_SPELLS: &str = "/lol-game-data/assets/v1/summoner-spells.json";
pub const PATH_GD_PERKS: &str = "/lol-game-data/assets/v1/perks.json";
pub const PATH_GD_AUGMENTS: &str = "/lol-game-data/assets/v1/cherry-augments.json";
pub const PATH_GD_PROFILE_ICON: &str = "/lol-game-data/assets/v1/profile-icons/%d.jpg";
pub const PATH_GD_CHAMPION_ICON: &str = "/lol-game-data/assets/v1/champion-icons/%d.png";
pub const PATH_GD_ITEM_ICON: &str = "/lol-game-data/assets/v1/items/icons2d/%d.png";
pub const PATH_GD_CHAMPION_SUMMARY: &str = "/lol-game-data/assets/v1/champion-summary.json";

/// Live Client Data API（仅游戏进程中可达，127.0.0.1:2999）
pub const LIVE_CLIENT_BASE: &str = "https://127.0.0.1:2999";
pub const PATH_LIVE_ACTIVE_PLAYER: &str = "/liveclientdata/activeplayer";
pub const PATH_LIVE_PLAYER_LIST: &str = "/liveclientdata/playerlist";
pub const PATH_LIVE_GAME_STATS: &str = "/liveclientdata/gamestats";
pub const PATH_LIVE_EVENTS: &str = "/liveclientdata/eventdata";
pub const PATH_LIVE_ACTIVE_PLAYER_NAME: &str = "/liveclientdata/activeplayername";

/// WS 订阅白名单（对齐 watchedURIs，前缀匹配）
pub const WATCHED_URIS: &[&str] = &[
    PATH_GAMEFLOW_PHASE,
    PATH_GAMEFLOW_SESSION,
    PATH_CHAMP_SELECT_SESSION,
    PATH_CURRENT_SUMMONER,
    PATH_READY_CHECK,
    PATH_LOBBY,
];

/// 路径前缀白名单（对齐 Go allowedAPIPrefixes）
pub const PATH_PREFIX_ALLOWLIST: &[&str] = &[
    "/system/",
    "/lol-summoner/",
    "/lol-match-history/",
    "/lol-ranked/",
    "/lol-gameflow/",
    "/lol-champ-select/",
    "/lol-game-data/",
    "/lol-lobby/",
    "/lol-spectator/",
    "/lol-matchmaking/",
    "/lol-league-session/",
    "/entitlements/",
    "/fe/lol-loot/",
];

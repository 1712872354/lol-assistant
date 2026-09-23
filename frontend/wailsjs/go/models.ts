export namespace config {
	
	export class Config {
	    schemaVersion: number;
	    theme: string;
	    pageSize: number;
	    sgpEnabled: boolean;
	    closeToTray: boolean;
	    clientPath: string;
	
	    static createFrom(source: any = {}) {
	        return new Config(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.schemaVersion = source["schemaVersion"];
	        this.theme = source["theme"];
	        this.pageSize = source["pageSize"];
	        this.sgpEnabled = source["sgpEnabled"];
	        this.closeToTray = source["closeToTray"];
	        this.clientPath = source["clientPath"];
	    }
	}

}

export namespace gameinfo {
	
	export class RecentMatch {
	    queueShort: string;
	    queueName: string;
	    timeShort: string;
	    gameCreation: number;
	    win: boolean;
	    kills: number;
	    deaths: number;
	    assists: number;
	    championId: number;
	
	    static createFrom(source: any = {}) {
	        return new RecentMatch(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.queueShort = source["queueShort"];
	        this.queueName = source["queueName"];
	        this.timeShort = source["timeShort"];
	        this.gameCreation = source["gameCreation"];
	        this.win = source["win"];
	        this.kills = source["kills"];
	        this.deaths = source["deaths"];
	        this.assists = source["assists"];
	        this.championId = source["championId"];
	    }
	}
	export class PlayerSlot {
	    filled: boolean;
	    isSelf: boolean;
	    puuid?: string;
	    summonerId?: string;
	    gameName?: string;
	    tagLine?: string;
	    profileIconId?: number;
	    championId?: number;
	    solo?: string;
	    flex?: string;
	    winRate?: number;
	    winRateSample?: number;
	    avgKda?: number;
	    rating?: number;
	    hiddenCareer?: boolean;
	    recent?: RecentMatch[];
	
	    static createFrom(source: any = {}) {
	        return new PlayerSlot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.filled = source["filled"];
	        this.isSelf = source["isSelf"];
	        this.puuid = source["puuid"];
	        this.summonerId = source["summonerId"];
	        this.gameName = source["gameName"];
	        this.tagLine = source["tagLine"];
	        this.profileIconId = source["profileIconId"];
	        this.championId = source["championId"];
	        this.solo = source["solo"];
	        this.flex = source["flex"];
	        this.winRate = source["winRate"];
	        this.winRateSample = source["winRateSample"];
	        this.avgKda = source["avgKda"];
	        this.rating = source["rating"];
	        this.hiddenCareer = source["hiddenCareer"];
	        this.recent = this.convertValues(source["recent"], RecentMatch);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	
	export class TeamView {
	    key: string;
	    label: string;
	    sideText: string;
	    badge: string;
	    playerCount: number;
	    phaseLabel: string;
	    winRate: number;
	    compScore: number;
	    rating: number;
	    slots: PlayerSlot[];
	
	    static createFrom(source: any = {}) {
	        return new TeamView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.key = source["key"];
	        this.label = source["label"];
	        this.sideText = source["sideText"];
	        this.badge = source["badge"];
	        this.playerCount = source["playerCount"];
	        this.phaseLabel = source["phaseLabel"];
	        this.winRate = source["winRate"];
	        this.compScore = source["compScore"];
	        this.rating = source["rating"];
	        this.slots = this.convertValues(source["slots"], PlayerSlot);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class ViewState {
	    phase: string;
	    queueLabel: string;
	    queueId: number;
	    teams: TeamView[];
	
	    static createFrom(source: any = {}) {
	        return new ViewState(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.phase = source["phase"];
	        this.queueLabel = source["queueLabel"];
	        this.queueId = source["queueId"];
	        this.teams = this.convertValues(source["teams"], TeamView);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

export namespace history {
	
	export class AssetResult {
	    kind: string;
	    id: number;
	    mime: string;
	    data: string;
	
	    static createFrom(source: any = {}) {
	        return new AssetResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.kind = source["kind"];
	        this.id = source["id"];
	        this.mime = source["mime"];
	        this.data = source["data"];
	    }
	}
	export class MatchPage {
	    puuid: string;
	    page: number;
	    pageSize: number;
	    gameCount: number;
	    totalPages: number;
	    hasMore: boolean;
	    summaries: parser.MatchSummary[];
	
	    static createFrom(source: any = {}) {
	        return new MatchPage(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.puuid = source["puuid"];
	        this.page = source["page"];
	        this.pageSize = source["pageSize"];
	        this.gameCount = source["gameCount"];
	        this.totalPages = source["totalPages"];
	        this.hasMore = source["hasMore"];
	        this.summaries = this.convertValues(source["summaries"], parser.MatchSummary);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class RankedInfo {
	    summonerId: string;
	    puuid?: string;
	    solo: string;
	    flex: string;
	
	    static createFrom(source: any = {}) {
	        return new RankedInfo(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.summonerId = source["summonerId"];
	        this.puuid = source["puuid"];
	        this.solo = source["solo"];
	        this.flex = source["flex"];
	    }
	}
	export class SummonerResult {
	    puuid: string;
	    gameName: string;
	    tagLine: string;
	    displayName: string;
	    profileIconId: number;
	    summonerLevel: number;
	    summonerId: string;
	
	    static createFrom(source: any = {}) {
	        return new SummonerResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.puuid = source["puuid"];
	        this.gameName = source["gameName"];
	        this.tagLine = source["tagLine"];
	        this.displayName = source["displayName"];
	        this.profileIconId = source["profileIconId"];
	        this.summonerLevel = source["summonerLevel"];
	        this.summonerId = source["summonerId"];
	    }
	}

}

export namespace lcu {
	
	export class ConnStatus {
	    state: string;
	    gameName?: string;
	    tagLine?: string;
	    platformId?: string;
	    summonerLevel?: number;
	    profileIconId?: number;
	    puuid?: string;
	
	    static createFrom(source: any = {}) {
	        return new ConnStatus(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.state = source["state"];
	        this.gameName = source["gameName"];
	        this.tagLine = source["tagLine"];
	        this.platformId = source["platformId"];
	        this.summonerLevel = source["summonerLevel"];
	        this.profileIconId = source["profileIconId"];
	        this.puuid = source["puuid"];
	    }
	}

}

export namespace parser {
	
	export class PlayerRow {
	    participantId: number;
	    teamId: number;
	    placement: number;
	    puuid: string;
	    summonerId: string;
	    name: string;
	    profileIconId: number;
	    championId: number;
	    champLevel: number;
	    spell1Id: number;
	    spell2Id: number;
	    runeId: number;
	    kills: number;
	    deaths: number;
	    assists: number;
	    kda: string;
	    items: number[];
	    cs: number;
	    gold: number;
	    totalDamage: number;
	    totalHeal: number;
	    win: boolean;
	    remake: boolean;
	    augmentIds: number[];
	    tierShort: string;
	    dmgRatio: number;
	    rating: number;
	    ratingRank: number;
	    killPct: number;
	    isSelf: boolean;
	
	    static createFrom(source: any = {}) {
	        return new PlayerRow(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.participantId = source["participantId"];
	        this.teamId = source["teamId"];
	        this.placement = source["placement"];
	        this.puuid = source["puuid"];
	        this.summonerId = source["summonerId"];
	        this.name = source["name"];
	        this.profileIconId = source["profileIconId"];
	        this.championId = source["championId"];
	        this.champLevel = source["champLevel"];
	        this.spell1Id = source["spell1Id"];
	        this.spell2Id = source["spell2Id"];
	        this.runeId = source["runeId"];
	        this.kills = source["kills"];
	        this.deaths = source["deaths"];
	        this.assists = source["assists"];
	        this.kda = source["kda"];
	        this.items = source["items"];
	        this.cs = source["cs"];
	        this.gold = source["gold"];
	        this.totalDamage = source["totalDamage"];
	        this.totalHeal = source["totalHeal"];
	        this.win = source["win"];
	        this.remake = source["remake"];
	        this.augmentIds = source["augmentIds"];
	        this.tierShort = source["tierShort"];
	        this.dmgRatio = source["dmgRatio"];
	        this.rating = source["rating"];
	        this.ratingRank = source["ratingRank"];
	        this.killPct = source["killPct"];
	        this.isSelf = source["isSelf"];
	    }
	}
	export class TeamSummary {
	    teamId: number;
	    placement: number;
	    win: boolean;
	    kills: number;
	    deaths: number;
	    assists: number;
	    gold: number;
	    damage: number;
	    players: PlayerRow[];
	
	    static createFrom(source: any = {}) {
	        return new TeamSummary(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.teamId = source["teamId"];
	        this.placement = source["placement"];
	        this.win = source["win"];
	        this.kills = source["kills"];
	        this.deaths = source["deaths"];
	        this.assists = source["assists"];
	        this.gold = source["gold"];
	        this.damage = source["damage"];
	        this.players = this.convertValues(source["players"], PlayerRow);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class MatchDetail {
	    gameId: number;
	    queueId: number;
	    queueName: string;
	    mapName: string;
	    arena: boolean;
	    gameCreation: number;
	    time: string;
	    gameDuration: number;
	    duration: string;
	    durationMin: string;
	    remake: boolean;
	    selfPuuid: string;
	    selfTeamIndex: number;
	    teams: TeamSummary[];
	
	    static createFrom(source: any = {}) {
	        return new MatchDetail(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.gameId = source["gameId"];
	        this.queueId = source["queueId"];
	        this.queueName = source["queueName"];
	        this.mapName = source["mapName"];
	        this.arena = source["arena"];
	        this.gameCreation = source["gameCreation"];
	        this.time = source["time"];
	        this.gameDuration = source["gameDuration"];
	        this.duration = source["duration"];
	        this.durationMin = source["durationMin"];
	        this.remake = source["remake"];
	        this.selfPuuid = source["selfPuuid"];
	        this.selfTeamIndex = source["selfTeamIndex"];
	        this.teams = this.convertValues(source["teams"], TeamSummary);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class MatchSummary {
	    gameId: number;
	    queueId: number;
	    queueName: string;
	    queueShort: string;
	    mapName: string;
	    arena: boolean;
	    gameCreation: number;
	    gameDuration: number;
	    time: string;
	    shortTime: string;
	    duration: string;
	    championId: number;
	    champLevel: number;
	    spell1Id: number;
	    spell2Id: number;
	    runeId: number;
	    kills: number;
	    deaths: number;
	    assists: number;
	    kda: string;
	    win: boolean;
	    remake: boolean;
	    placement: number;
	    items: number[];
	    cs: number;
	    gold: number;
	    totalDamage: number;
	    totalHeal: number;
	    augmentIds: number[];
	    teamId: number;
	
	    static createFrom(source: any = {}) {
	        return new MatchSummary(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.gameId = source["gameId"];
	        this.queueId = source["queueId"];
	        this.queueName = source["queueName"];
	        this.queueShort = source["queueShort"];
	        this.mapName = source["mapName"];
	        this.arena = source["arena"];
	        this.gameCreation = source["gameCreation"];
	        this.gameDuration = source["gameDuration"];
	        this.time = source["time"];
	        this.shortTime = source["shortTime"];
	        this.duration = source["duration"];
	        this.championId = source["championId"];
	        this.champLevel = source["champLevel"];
	        this.spell1Id = source["spell1Id"];
	        this.spell2Id = source["spell2Id"];
	        this.runeId = source["runeId"];
	        this.kills = source["kills"];
	        this.deaths = source["deaths"];
	        this.assists = source["assists"];
	        this.kda = source["kda"];
	        this.win = source["win"];
	        this.remake = source["remake"];
	        this.placement = source["placement"];
	        this.items = source["items"];
	        this.cs = source["cs"];
	        this.gold = source["gold"];
	        this.totalDamage = source["totalDamage"];
	        this.totalHeal = source["totalHeal"];
	        this.augmentIds = source["augmentIds"];
	        this.teamId = source["teamId"];
	    }
	}
	

}

export namespace update {
	
	export class Info {
	    hasUpdate: boolean;
	    currentVersion: string;
	    version: string;
	    notes: string;
	    pubDate: string;
	    setupUrl: string;
	    portableUrl: string;
	    sha256: string;
	    releaseUrl: string;
	
	    static createFrom(source: any = {}) {
	        return new Info(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.hasUpdate = source["hasUpdate"];
	        this.currentVersion = source["currentVersion"];
	        this.version = source["version"];
	        this.notes = source["notes"];
	        this.pubDate = source["pubDate"];
	        this.setupUrl = source["setupUrl"];
	        this.portableUrl = source["portableUrl"];
	        this.sha256 = source["sha256"];
	        this.releaseUrl = source["releaseUrl"];
	    }
	}

}


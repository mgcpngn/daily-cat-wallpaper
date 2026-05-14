import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type Locale = "en" | "zh-Hans" | "zh-Hant" | "ja" | "ko";
type LanguagePreference =
  | "Auto"
  | "English"
  | "SimplifiedChinese"
  | "TraditionalChinese"
  | "Japanese"
  | "Korean";
type CatCountStrategy = "MatchDisplays" | "Fixed";
type CatImageType = "Healing" | "Funny" | "Loaf" | "Kitten" | "Sleepy";
type PlatformMode = "Automatic" | "StaticOnly" | "InteractionBeta";
type ScheduleConfig =
  | "OnLogin"
  | "ManualOnly"
  | { Daily: { time: string } }
  | { EveryHours: { hours: number } };

type AppConfig = {
  language: LanguagePreference;
  breeds: string[];
  cat_count_strategy: CatCountStrategy;
  cat_count: number;
  image_types: CatImageType[];
  image_quality: {
    min_width: number;
    min_height: number;
    preferred_width: number;
    preferred_height: number;
    allow_low_resolution_fallback: boolean;
  };
  interactions: {
    breathing: boolean;
    mouse_proximity: boolean;
    click_paw: boolean;
    keyboard_bongo: boolean;
    sound: boolean;
  };
  schedule: ScheduleConfig;
  sources: {
    local_dirs: string[];
    wikimedia_commons: boolean;
    cataas: boolean;
    the_cat_api: boolean;
    pixabay_api_key: string | null;
    magnific_api_key: string | null;
    pexels_api_key: string | null;
    unsplash_access_key: string | null;
  };
  platform_mode: PlatformMode;
  launch_at_login: boolean;
};

type Capabilities = {
  platform: string;
  static_wallpaper: boolean;
  interaction_overlay: boolean;
  supported_interactions: string[];
  beta: boolean;
};

type Rect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type DisplayGeometry = {
  x: number;
  y: number;
  width: number;
  height: number;
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const BREEDS = [
  "mixed",
  "orange tabby",
  "british shorthair",
  "ragdoll",
  "maine coon",
  "siamese",
  "black cat",
  "calico",
  "white cat",
];

const IMAGE_TYPES: CatImageType[] = ["Healing", "Funny", "Loaf", "Kitten", "Sleepy"];
const LANGUAGES: Array<{ value: LanguagePreference; labelKey: string }> = [
  { value: "Auto", labelKey: "language.auto" },
  { value: "English", labelKey: "language.en" },
  { value: "SimplifiedChinese", labelKey: "language.zhHans" },
  { value: "TraditionalChinese", labelKey: "language.zhHant" },
  { value: "Japanese", labelKey: "language.ja" },
  { value: "Korean", labelKey: "language.ko" },
];

const defaultConfig: AppConfig = {
  language: "Auto",
  breeds: ["mixed"],
  cat_count_strategy: "MatchDisplays",
  cat_count: 1,
  image_types: ["Healing", "Loaf"],
  image_quality: {
    min_width: 2560,
    min_height: 1440,
    preferred_width: 3840,
    preferred_height: 2160,
    allow_low_resolution_fallback: false,
  },
  interactions: {
    breathing: true,
    mouse_proximity: true,
    click_paw: false,
    keyboard_bongo: false,
    sound: false,
  },
  schedule: { Daily: { time: "09:00" } },
  sources: {
    local_dirs: [],
    wikimedia_commons: true,
    cataas: true,
    the_cat_api: true,
    pixabay_api_key: null,
    magnific_api_key: null,
    pexels_api_key: null,
    unsplash_access_key: null,
  },
  platform_mode: "Automatic",
  launch_at_login: true,
};

const dictionary: Record<Locale, Record<string, string>> = {
  en: {
    "app.name": "Daily Cat Wallpaper",
    "hero.title": "Cat-powered desktop control.",
    "hero.lead":
      "Choose language, cat sources, HD quality, monitor behavior, and interaction style. The Rust core takes over wallpaper refreshes.",
    "action.refresh": "Refresh now",
    "action.prefetch": "Cache HD pack",
    "action.save": "Save preferences",
    "status.label": "Status",
    "status.loading": "Loading configuration",
    "status.ready": "Ready",
    "status.preview": "Preview mode: open the Tauri app to change wallpaper.",
    "status.saving": "Saving preferences",
    "status.saved": "Preferences saved",
    "status.refreshing": "Refreshing wallpaper",
    "status.prefetching": "Caching HD cat pack",
    "status.prefetched": "Cached {count} HD cat images",
    "status.wallpaperSet": "Wallpaper set: {path}",
    "status.platformDetecting": "Detecting platform",
    "status.platform": "{platform}{beta} / static {static}",
    "status.readyWord": "ready",
    "status.unavailableWord": "unavailable",
    "preview.title.match": "{count} displays, {cats} different cats",
    "preview.title.fixed": "{count} displays, fixed {cats} cat setting",
    "preview.copy":
      "Safe layout keeps the left icon area and taskbar clear. Active: {interactions}.",
    "preview.none": "none",
    "panel.language": "Language",
    "panel.identity": "Cat identity",
    "panel.quality": "Image quality",
    "panel.mood": "Image mood",
    "panel.interactions": "Interactions",
    "panel.schedule": "Refresh frequency",
    "panel.sources": "Sources",
    "panel.platform": "Platform",
    "field.language": "Display language",
    "language.auto": "Auto from system locale",
    "language.en": "English",
    "language.zhHans": "简体中文",
    "language.zhHant": "繁體中文",
    "language.ja": "日本語",
    "language.ko": "한국어",
    "field.breeds": "Breed preferences",
    "field.countStrategy": "Monitor behavior",
    "count.match": "Match displays",
    "count.fixed": "Fixed count",
    "field.catCount": "Fixed cat count",
    "hint.matchDisplays": "Default: one different cat per display.",
    "hint.fixedOne": "When fixed to 1, the same cat is reused on every display.",
    "field.minResolution": "Minimum accepted resolution",
    "field.preferredResolution": "Preferred online request size",
    "field.lowResFallback": "Allow low-resolution fallback",
    "hint.lowResFallback": "Keep disabled for HD-only wallpapers.",
    "schedule.daily": "Daily",
    "schedule.everyHours": "Every N hours",
    "schedule.onLogin": "On login",
    "schedule.manual": "Manual",
    "field.dailyTime": "Daily time",
    "field.everyHours": "Every N hours",
    "source.cataas": "Random no-key cats requested at preferred size",
    "source.theCatApi": "Full-size cat candidates and breed metadata",
    "source.wikimedia": "Stable HD images matched by breed and color keywords",
    "source.pixabay": "Optional API key source for public HD cat photos",
    "source.magnific": "Optional API key source for Magnific HD resources",
    "source.pexels": "Optional API key source for curated HD cat photos",
    "field.pixabayKey": "Pixabay API key",
    "field.magnificKey": "Magnific API key",
    "field.pexelsKey": "Pexels API key",
    "source.getPexelsKey": "Get Pexels API key",
    "source.localPlaceholder": "C:\\Users\\you\\Pictures\\Cats",
    "source.add": "Add",
    "field.mode": "Mode",
    "mode.automatic": "Automatic",
    "mode.static": "Static only",
    "mode.beta": "Interaction beta",
    "field.launch": "Launch at login",
    "hint.launch": "Refresh cats when the desktop starts",
    "interaction.breathing": "Breathing",
    "interaction.mouse_proximity": "Mouse proximity",
    "interaction.click_paw": "Click paw",
    "interaction.keyboard_bongo": "Keyboard Bongo",
    "interaction.sound": "Sound",
    "hint.breathing": "Subtle motion on generated cat layers",
    "hint.mouse_proximity": "Cats react when the pointer gets close",
    "hint.click_paw": "Click feedback for cat paws",
    "hint.keyboard_bongo": "Keyboard rhythm reaction",
    "hint.sound": "Optional short sound effects",
    "hint.optional": "Optional behavior",
    "breed.mixed": "mixed",
    "breed.orange tabby": "orange tabby",
    "breed.british shorthair": "british shorthair",
    "breed.ragdoll": "ragdoll",
    "breed.maine coon": "maine coon",
    "breed.siamese": "siamese",
    "breed.black cat": "black cat",
    "breed.calico": "calico",
    "breed.white cat": "white cat",
    "image.Healing": "Healing",
    "image.Funny": "Funny",
    "image.Loaf": "Loaf",
    "image.Kitten": "Kitten",
    "image.Sleepy": "Sleepy",
  },
  "zh-Hans": {
    "app.name": "每日吸猫壁纸",
    "hero.title": "让猫咪接管你的桌面。",
    "hero.lead": "选择语言、猫图来源、高清质量、多屏策略和互动方式。Rust 核心负责自动刷新和壁纸接管。",
    "action.refresh": "立即换猫",
    "action.prefetch": "缓存高清猫图包",
    "action.save": "保存配置",
    "status.label": "状态",
    "status.loading": "正在加载配置",
    "status.ready": "已就绪",
    "status.preview": "预览模式：请打开 Tauri 应用来真正更换壁纸。",
    "status.saving": "正在保存偏好",
    "status.saved": "偏好已保存",
    "status.refreshing": "正在刷新壁纸",
    "status.prefetching": "正在缓存高清猫图包",
    "status.prefetched": "已缓存 {count} 张高清猫图",
    "status.wallpaperSet": "壁纸已设置：{path}",
    "status.platformDetecting": "正在检测平台",
    "status.platform": "{platform}{beta} / 静态壁纸 {static}",
    "status.readyWord": "可用",
    "status.unavailableWord": "不可用",
    "preview.title.match": "{count} 块屏幕，{cats} 只不同猫咪",
    "preview.title.fixed": "{count} 块屏幕，固定 {cats} 只猫",
    "preview.copy": "布局会避开左侧图标区和任务栏。当前互动：{interactions}。",
    "preview.none": "无",
    "panel.language": "语言",
    "panel.identity": "猫咪身份",
    "panel.quality": "图片质量",
    "panel.mood": "图片类型",
    "panel.interactions": "互动",
    "panel.schedule": "刷新频次",
    "panel.sources": "图片来源",
    "panel.platform": "平台",
    "field.language": "显示语言",
    "language.auto": "跟随系统语言",
    "language.en": "English",
    "language.zhHans": "简体中文",
    "language.zhHant": "繁體中文",
    "language.ja": "日本語",
    "language.ko": "한국어",
    "field.breeds": "品种偏好",
    "field.countStrategy": "多屏策略",
    "count.match": "跟随屏幕数",
    "count.fixed": "固定数量",
    "field.catCount": "固定猫咪数量",
    "hint.matchDisplays": "默认：几块屏幕就放几只不同的猫。",
    "hint.fixedOne": "固定为 1 时，所有屏幕复用同一只猫。",
    "field.minResolution": "最低接受分辨率",
    "field.preferredResolution": "在线请求优先尺寸",
    "field.lowResFallback": "允许低分辨率兜底",
    "hint.lowResFallback": "关闭时只接受高清壁纸。",
    "schedule.daily": "每天",
    "schedule.everyHours": "每 N 小时",
    "schedule.onLogin": "登录时",
    "schedule.manual": "手动",
    "field.dailyTime": "每天时间",
    "field.everyHours": "间隔小时",
    "source.cataas": "免 Key 随机猫图，按优先尺寸请求",
    "source.theCatApi": "全尺寸候选猫图和品种元数据",
    "source.wikimedia": "按品种和花色关键词匹配的稳定高清图片",
    "source.pixabay": "可选 API Key 来源，获取公开高清猫图",
    "source.magnific": "可选 API Key 来源，获取 Magnific 高清资源",
    "source.pexels": "可选 API Key 来源，获取精选高清猫图",
    "field.pixabayKey": "Pixabay API Key",
    "field.magnificKey": "Magnific API Key",
    "field.pexelsKey": "Pexels API Key",
    "source.getPexelsKey": "获取 Pexels API Key",
    "source.localPlaceholder": "C:\\Users\\you\\Pictures\\Cats",
    "source.add": "添加",
    "field.mode": "模式",
    "mode.automatic": "自动",
    "mode.static": "仅静态",
    "mode.beta": "互动 Beta",
    "field.launch": "开机启动",
    "hint.launch": "桌面启动时自动刷新猫图",
    "interaction.breathing": "呼吸动效",
    "interaction.mouse_proximity": "鼠标靠近",
    "interaction.click_paw": "点击猫爪",
    "interaction.keyboard_bongo": "键盘 Bongo",
    "interaction.sound": "声音",
    "hint.breathing": "生成猫图层的轻微动效",
    "hint.mouse_proximity": "指针靠近时猫咪响应",
    "hint.click_paw": "点击猫爪反馈",
    "hint.keyboard_bongo": "跟随键盘节奏反应",
    "hint.sound": "可选短音效",
    "hint.optional": "可选行为",
    "breed.mixed": "混血猫",
    "breed.orange tabby": "橘猫",
    "breed.british shorthair": "英国短毛猫",
    "breed.ragdoll": "布偶猫",
    "breed.maine coon": "缅因猫",
    "breed.siamese": "暹罗猫",
    "breed.black cat": "黑猫",
    "breed.calico": "三花猫",
    "breed.white cat": "白猫",
    "image.Healing": "治愈",
    "image.Funny": "搞笑",
    "image.Loaf": "猫 loaf",
    "image.Kitten": "幼猫",
    "image.Sleepy": "睡觉",
  },
  "zh-Hant": {
    "app.name": "每日吸貓桌布",
    "hero.title": "讓貓咪接管你的桌面。",
    "hero.lead": "選擇語言、貓圖來源、高清品質、多螢幕策略和互動方式。Rust 核心負責自動刷新和桌布接管。",
    "action.refresh": "立即換貓",
    "action.prefetch": "快取高清貓圖包",
    "action.save": "儲存設定",
    "status.label": "狀態",
    "status.loading": "正在載入設定",
    "status.ready": "已就緒",
    "status.preview": "預覽模式：請開啟 Tauri 應用程式來真正更換桌布。",
    "status.saving": "正在儲存偏好",
    "status.saved": "偏好已儲存",
    "status.refreshing": "正在刷新桌布",
    "status.prefetching": "正在快取高清貓圖包",
    "status.prefetched": "已快取 {count} 張高清貓圖",
    "status.wallpaperSet": "桌布已設定：{path}",
    "status.platformDetecting": "正在偵測平台",
    "status.platform": "{platform}{beta} / 靜態桌布 {static}",
    "status.readyWord": "可用",
    "status.unavailableWord": "不可用",
    "preview.title.match": "{count} 個螢幕，{cats} 隻不同貓咪",
    "preview.title.fixed": "{count} 個螢幕，固定 {cats} 隻貓",
    "preview.copy": "版面會避開左側圖示區和工作列。目前互動：{interactions}。",
    "preview.none": "無",
    "panel.language": "語言",
    "panel.identity": "貓咪身份",
    "panel.quality": "圖片品質",
    "panel.mood": "圖片類型",
    "panel.interactions": "互動",
    "panel.schedule": "刷新頻率",
    "panel.sources": "圖片來源",
    "panel.platform": "平台",
    "field.language": "顯示語言",
    "language.auto": "跟隨系統語言",
    "language.en": "English",
    "language.zhHans": "简体中文",
    "language.zhHant": "繁體中文",
    "language.ja": "日本語",
    "language.ko": "한국어",
    "field.breeds": "品種偏好",
    "field.countStrategy": "多螢幕策略",
    "count.match": "跟隨螢幕數",
    "count.fixed": "固定數量",
    "field.catCount": "固定貓咪數量",
    "hint.matchDisplays": "預設：幾個螢幕就放幾隻不同的貓。",
    "hint.fixedOne": "固定為 1 時，所有螢幕會使用同一隻貓。",
    "field.minResolution": "最低接受解析度",
    "field.preferredResolution": "線上請求優先尺寸",
    "field.lowResFallback": "允許低解析度備援",
    "hint.lowResFallback": "關閉時只接受高清桌布。",
    "schedule.daily": "每天",
    "schedule.everyHours": "每 N 小時",
    "schedule.onLogin": "登入時",
    "schedule.manual": "手動",
    "field.dailyTime": "每天時間",
    "field.everyHours": "間隔小時",
    "source.cataas": "免 Key 隨機貓圖，依優先尺寸請求",
    "source.theCatApi": "全尺寸候選貓圖和品種 metadata",
    "source.wikimedia": "依品種和花色關鍵字匹配的穩定高清圖片",
    "source.pixabay": "可選 API Key 來源，取得公開高清貓圖",
    "source.magnific": "可選 API Key 來源，取得 Magnific 高清資源",
    "source.pexels": "可選 API Key 來源，取得精選高清貓圖",
    "field.pixabayKey": "Pixabay API Key",
    "field.magnificKey": "Magnific API Key",
    "field.pexelsKey": "Pexels API Key",
    "source.getPexelsKey": "取得 Pexels API Key",
    "source.localPlaceholder": "C:\\Users\\you\\Pictures\\Cats",
    "source.add": "新增",
    "field.mode": "模式",
    "mode.automatic": "自動",
    "mode.static": "僅靜態",
    "mode.beta": "互動 Beta",
    "field.launch": "開機啟動",
    "hint.launch": "桌面啟動時自動刷新貓圖",
    "interaction.breathing": "呼吸動效",
    "interaction.mouse_proximity": "滑鼠靠近",
    "interaction.click_paw": "點擊貓掌",
    "interaction.keyboard_bongo": "鍵盤 Bongo",
    "interaction.sound": "聲音",
    "hint.breathing": "生成貓圖層的輕微動效",
    "hint.mouse_proximity": "游標靠近時貓咪回應",
    "hint.click_paw": "點擊貓掌回饋",
    "hint.keyboard_bongo": "跟隨鍵盤節奏反應",
    "hint.sound": "可選短音效",
    "hint.optional": "可選行為",
    "breed.mixed": "混種貓",
    "breed.orange tabby": "橘貓",
    "breed.british shorthair": "英國短毛貓",
    "breed.ragdoll": "布偶貓",
    "breed.maine coon": "緬因貓",
    "breed.siamese": "暹羅貓",
    "breed.black cat": "黑貓",
    "breed.calico": "三花貓",
    "breed.white cat": "白貓",
    "image.Healing": "療癒",
    "image.Funny": "搞笑",
    "image.Loaf": "貓 loaf",
    "image.Kitten": "幼貓",
    "image.Sleepy": "睡覺",
  },
  ja: {
    "app.name": "Daily Cat Wallpaper",
    "hero.title": "猫がデスクトップを支配します。",
    "hero.lead": "言語、猫画像ソース、HD 品質、マルチモニター動作、インタラクションを選べます。Rust コアが壁紙更新を担当します。",
    "action.refresh": "今すぐ更新",
    "action.prefetch": "HD パックを保存",
    "action.save": "設定を保存",
    "status.label": "状態",
    "status.loading": "設定を読み込み中",
    "status.ready": "準備完了",
    "status.preview": "プレビューモード: 壁紙変更には Tauri アプリを開いてください。",
    "status.saving": "設定を保存中",
    "status.saved": "設定を保存しました",
    "status.refreshing": "壁紙を更新中",
    "status.prefetching": "HD 猫画像パックを保存中",
    "status.prefetched": "{count} 枚の HD 猫画像を保存しました",
    "status.wallpaperSet": "壁紙を設定しました: {path}",
    "status.platformDetecting": "プラットフォームを検出中",
    "status.platform": "{platform}{beta} / 静的壁紙 {static}",
    "status.readyWord": "利用可能",
    "status.unavailableWord": "利用不可",
    "preview.title.match": "{count} 台の画面に {cats} 匹の別々の猫",
    "preview.title.fixed": "{count} 台の画面、固定 {cats} 匹設定",
    "preview.copy": "左側のアイコン領域とタスクバーを避けます。現在のインタラクション: {interactions}。",
    "preview.none": "なし",
    "panel.language": "言語",
    "panel.identity": "猫の種類",
    "panel.quality": "画像品質",
    "panel.mood": "画像タイプ",
    "panel.interactions": "インタラクション",
    "panel.schedule": "更新頻度",
    "panel.sources": "画像ソース",
    "panel.platform": "プラットフォーム",
    "field.language": "表示言語",
    "language.auto": "システム言語に合わせる",
    "language.en": "English",
    "language.zhHans": "简体中文",
    "language.zhHant": "繁體中文",
    "language.ja": "日本語",
    "language.ko": "한국어",
    "field.breeds": "品種の好み",
    "field.countStrategy": "モニター動作",
    "count.match": "画面数に合わせる",
    "count.fixed": "固定数",
    "field.catCount": "固定の猫数",
    "hint.matchDisplays": "既定: 画面ごとに別の猫を表示します。",
    "hint.fixedOne": "1 匹に固定すると、すべての画面で同じ猫を使います。",
    "field.minResolution": "最低解像度",
    "field.preferredResolution": "オンライン取得の優先サイズ",
    "field.lowResFallback": "低解像度フォールバックを許可",
    "hint.lowResFallback": "HD のみ使う場合はオフにします。",
    "schedule.daily": "毎日",
    "schedule.everyHours": "N 時間ごと",
    "schedule.onLogin": "ログイン時",
    "schedule.manual": "手動",
    "field.dailyTime": "毎日の時刻",
    "field.everyHours": "間隔時間",
    "source.cataas": "キー不要のランダム猫画像を優先サイズで取得",
    "source.theCatApi": "フルサイズ候補と品種メタデータ",
    "source.wikimedia": "品種と毛色キーワードで一致する安定した HD 画像",
    "source.pixabay": "公開 HD 猫写真用の任意 API キーソース",
    "source.magnific": "Magnific HD リソース用の任意 API キーソース",
    "source.pexels": "厳選 HD 猫写真用の任意 API キーソース",
    "field.pixabayKey": "Pixabay API Key",
    "field.magnificKey": "Magnific API Key",
    "field.pexelsKey": "Pexels API Key",
    "source.getPexelsKey": "Pexels API Key を取得",
    "source.localPlaceholder": "C:\\Users\\you\\Pictures\\Cats",
    "source.add": "追加",
    "field.mode": "モード",
    "mode.automatic": "自動",
    "mode.static": "静的のみ",
    "mode.beta": "インタラクション Beta",
    "field.launch": "ログイン時に起動",
    "hint.launch": "デスクトップ起動時に猫画像を更新",
    "interaction.breathing": "呼吸",
    "interaction.mouse_proximity": "マウス接近",
    "interaction.click_paw": "肉球クリック",
    "interaction.keyboard_bongo": "キーボード Bongo",
    "interaction.sound": "音",
    "hint.breathing": "生成された猫レイヤーの微細な動き",
    "hint.mouse_proximity": "ポインター接近時に猫が反応",
    "hint.click_paw": "肉球クリックの反応",
    "hint.keyboard_bongo": "キー入力リズムへの反応",
    "hint.sound": "任意の短い効果音",
    "hint.optional": "任意の動作",
    "breed.mixed": "ミックス",
    "breed.orange tabby": "茶トラ",
    "breed.british shorthair": "ブリティッシュショートヘア",
    "breed.ragdoll": "ラグドール",
    "breed.maine coon": "メインクーン",
    "breed.siamese": "シャム",
    "breed.black cat": "黒猫",
    "breed.calico": "三毛猫",
    "breed.white cat": "白猫",
    "image.Healing": "癒やし",
    "image.Funny": "面白い",
    "image.Loaf": "香箱座り",
    "image.Kitten": "子猫",
    "image.Sleepy": "眠い",
  },
  ko: {
    "app.name": "Daily Cat Wallpaper",
    "hero.title": "고양이가 데스크톱을 접수합니다.",
    "hero.lead": "언어, 고양이 이미지 소스, HD 품질, 다중 모니터 방식, 상호작용을 선택하세요. Rust 코어가 배경화면 갱신을 처리합니다.",
    "action.refresh": "지금 새로고침",
    "action.prefetch": "HD 팩 캐시",
    "action.save": "설정 저장",
    "status.label": "상태",
    "status.loading": "설정 불러오는 중",
    "status.ready": "준비됨",
    "status.preview": "미리보기 모드: 배경화면 변경은 Tauri 앱에서 가능합니다.",
    "status.saving": "설정 저장 중",
    "status.saved": "설정 저장됨",
    "status.refreshing": "배경화면 새로고침 중",
    "status.prefetching": "HD 고양이 이미지 팩 캐시 중",
    "status.prefetched": "HD 고양이 이미지 {count}장 캐시됨",
    "status.wallpaperSet": "배경화면 설정됨: {path}",
    "status.platformDetecting": "플랫폼 감지 중",
    "status.platform": "{platform}{beta} / 정적 배경화면 {static}",
    "status.readyWord": "사용 가능",
    "status.unavailableWord": "사용 불가",
    "preview.title.match": "{count}개 화면, 서로 다른 고양이 {cats}마리",
    "preview.title.fixed": "{count}개 화면, 고정 {cats}마리 설정",
    "preview.copy": "왼쪽 아이콘 영역과 작업 표시줄을 피합니다. 활성 상호작용: {interactions}.",
    "preview.none": "없음",
    "panel.language": "언어",
    "panel.identity": "고양이 정체성",
    "panel.quality": "이미지 품질",
    "panel.mood": "이미지 유형",
    "panel.interactions": "상호작용",
    "panel.schedule": "새로고침 빈도",
    "panel.sources": "이미지 소스",
    "panel.platform": "플랫폼",
    "field.language": "표시 언어",
    "language.auto": "시스템 언어 따르기",
    "language.en": "English",
    "language.zhHans": "简体中文",
    "language.zhHant": "繁體中文",
    "language.ja": "日本語",
    "language.ko": "한국어",
    "field.breeds": "품종 선호",
    "field.countStrategy": "모니터 동작",
    "count.match": "화면 수에 맞춤",
    "count.fixed": "고정 수",
    "field.catCount": "고정 고양이 수",
    "hint.matchDisplays": "기본값: 화면마다 서로 다른 고양이를 표시합니다.",
    "hint.fixedOne": "1마리로 고정하면 모든 화면에서 같은 고양이를 사용합니다.",
    "field.minResolution": "최소 허용 해상도",
    "field.preferredResolution": "온라인 요청 우선 크기",
    "field.lowResFallback": "저해상도 대체 허용",
    "hint.lowResFallback": "HD 전용 배경화면은 꺼두세요.",
    "schedule.daily": "매일",
    "schedule.everyHours": "N시간마다",
    "schedule.onLogin": "로그인 시",
    "schedule.manual": "수동",
    "field.dailyTime": "매일 시간",
    "field.everyHours": "간격 시간",
    "source.cataas": "키 없이 무작위 고양이 이미지를 우선 크기로 요청",
    "source.theCatApi": "전체 크기 후보와 품종 메타데이터",
    "source.wikimedia": "품종과 털색 키워드에 맞춘 안정적인 HD 이미지",
    "source.pixabay": "공개 HD 고양이 사진용 선택 API 키 소스",
    "source.magnific": "Magnific HD 리소스용 선택 API 키 소스",
    "source.pexels": "큐레이션된 HD 고양이 사진용 선택 API 키 소스",
    "field.pixabayKey": "Pixabay API Key",
    "field.magnificKey": "Magnific API Key",
    "field.pexelsKey": "Pexels API Key",
    "source.getPexelsKey": "Pexels API Key 받기",
    "source.localPlaceholder": "C:\\Users\\you\\Pictures\\Cats",
    "source.add": "추가",
    "field.mode": "모드",
    "mode.automatic": "자동",
    "mode.static": "정적만",
    "mode.beta": "상호작용 Beta",
    "field.launch": "로그인 시 실행",
    "hint.launch": "데스크톱 시작 시 고양이 이미지 새로고침",
    "interaction.breathing": "숨쉬기",
    "interaction.mouse_proximity": "마우스 접근",
    "interaction.click_paw": "발바닥 클릭",
    "interaction.keyboard_bongo": "키보드 Bongo",
    "interaction.sound": "소리",
    "hint.breathing": "생성된 고양이 레이어의 은은한 움직임",
    "hint.mouse_proximity": "포인터가 가까워지면 고양이가 반응",
    "hint.click_paw": "발바닥 클릭 피드백",
    "hint.keyboard_bongo": "키보드 리듬 반응",
    "hint.sound": "선택적 짧은 효과음",
    "hint.optional": "선택 동작",
    "breed.mixed": "믹스",
    "breed.orange tabby": "치즈 태비",
    "breed.british shorthair": "브리티시 쇼트헤어",
    "breed.ragdoll": "랙돌",
    "breed.maine coon": "메인쿤",
    "breed.siamese": "샴",
    "breed.black cat": "검은 고양이",
    "breed.calico": "삼색 고양이",
    "breed.white cat": "흰 고양이",
    "image.Healing": "힐링",
    "image.Funny": "웃김",
    "image.Loaf": "식빵 자세",
    "image.Kitten": "아기 고양이",
    "image.Sleepy": "잠자는",
  },
};

function App() {
  const [config, setConfig] = useState<AppConfig>(defaultConfig);
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);
  const [displays, setDisplays] = useState<DisplayGeometry[]>(fallbackDisplays());
  const [slots, setSlots] = useState<Rect[]>([]);
  const [status, setStatus] = useState("status.loading");
  const [localDirInput, setLocalDirInput] = useState("");
  const locale = resolveLocale(config.language);
  const t = useMemo(() => translator(locale), [locale]);
  const scheduleKind = getScheduleKind(config.schedule);
  const displayCount = Math.max(displays.length, 1);
  const assignments = useMemo(
    () => catAssignments(displayCount, config),
    [displayCount, config.cat_count, config.cat_count_strategy],
  );
  const uniqueCatCount = Math.max(...assignments, 0) + 1;
  const previewSlotCount =
    config.cat_count_strategy === "Fixed" ? config.cat_count : Math.min(uniqueCatCount, 5);

  useEffect(() => {
    Promise.all([
      safeInvoke<AppConfig>("get_config", undefined, defaultConfig),
      safeInvoke<Capabilities | null>("platform_capabilities", undefined, null),
      safeInvoke<DisplayGeometry[]>("display_summary", undefined, fallbackDisplays()),
    ])
      .then(([loadedConfig, loadedCapabilities, loadedDisplays]) => {
        setConfig(mergeConfig(loadedConfig));
        setCapabilities(loadedCapabilities);
        setDisplays(loadedDisplays.length ? loadedDisplays : fallbackDisplays());
        setStatus(hasTauriRuntime() ? "status.ready" : "status.preview");
      })
      .catch((error) => setStatus(String(error)));
  }, []);

  useEffect(() => {
    safeInvoke<Rect[]>(
      "preview_layout",
      {
        catCount: previewSlotCount,
        width: 1920,
        height: 1080,
      },
      clientSlots(previewSlotCount),
    )
      .then(setSlots)
      .catch(() => setSlots(clientSlots(previewSlotCount)));
  }, [previewSlotCount]);

  const enabledInteractions = useMemo(
    () =>
      Object.entries(config.interactions)
        .filter(([, enabled]) => enabled)
        .map(([key]) => t(`interaction.${key}`)),
    [config.interactions, t],
  );

  async function save() {
    setStatus("status.saving");
    try {
      const saved = await safeInvoke<AppConfig>("save_config", { config }, config);
      setConfig(mergeConfig(saved));
      setStatus(hasTauriRuntime() ? "status.saved" : "status.preview");
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function refreshNow() {
    setStatus("status.refreshing");
    try {
      const path = await safeInvoke<string>("refresh_wallpaper", undefined, "");
      setStatus(
        path ? format(t("status.wallpaperSet"), { path }) : "status.preview",
      );
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function prefetchNow() {
    setStatus("status.prefetching");
    try {
      const paths = await safeInvoke<string[]>("prefetch_wallpapers", { count: 12 }, []);
      setStatus(format(t("status.prefetched"), { count: paths.length }));
    } catch (error) {
      setStatus(String(error));
    }
  }

  return (
    <main className="app-shell">
      <section className="hero">
        <div>
          <p className="eyebrow">{t("app.name")}</p>
          <h1>{t("hero.title")}</h1>
          <p className="lead">{t("hero.lead")}</p>
          <div className="action-row">
            <button className="primary" onClick={refreshNow}>
              {t("action.refresh")}
            </button>
            <button className="secondary" onClick={prefetchNow}>
              {t("action.prefetch")}
            </button>
            <button className="secondary" onClick={save}>
              {t("action.save")}
            </button>
          </div>
        </div>
        <div className="status-panel" aria-live="polite">
          <span className="status-label">{t("status.label")}</span>
          <strong>{status.startsWith("status.") ? t(status) : status}</strong>
          <span>
            {capabilities
              ? format(t("status.platform"), {
                  platform: capabilities.platform,
                  beta: capabilities.beta ? " beta" : "",
                  static: capabilities.static_wallpaper
                    ? t("status.readyWord")
                    : t("status.unavailableWord"),
                })
              : t("status.platformDetecting")}
          </span>
        </div>
      </section>

      <section className="workbench">
        <aside className="preview-pane">
          <div className="screen-preview">
            {slots.map((slot, index) => (
              <div
                className="cat-slot"
                key={`${slot.x}-${slot.y}-${index}`}
                style={{
                  left: `${(slot.x / 1920) * 100}%`,
                  top: `${(slot.y / 1080) * 100}%`,
                  width: `${(slot.width / 1920) * 100}%`,
                  height: `${(slot.height / 1080) * 100}%`,
                }}
              >
                <span>{index + 1}</span>
              </div>
            ))}
          </div>
          <div className="preview-copy">
            <h2>
              {format(
                t(
                  config.cat_count_strategy === "MatchDisplays"
                    ? "preview.title.match"
                    : "preview.title.fixed",
                ),
                { count: displayCount, cats: uniqueCatCount },
              )}
            </h2>
            <p>
              {format(t("preview.copy"), {
                interactions: enabledInteractions.length
                  ? enabledInteractions.join(", ")
                  : t("preview.none"),
              })}
            </p>
          </div>
        </aside>

        <section className="settings-grid">
          <Panel title={t("panel.language")}>
            <label className="field">
              {t("field.language")}
              <select
                value={config.language}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    language: event.currentTarget.value as LanguagePreference,
                  })
                }
              >
                {LANGUAGES.map((language) => (
                  <option key={language.value} value={language.value}>
                    {t(language.labelKey)}
                  </option>
                ))}
              </select>
            </label>
          </Panel>

          <Panel title={t("panel.identity")}>
            <label className="field">
              {t("field.breeds")}
              <select
                multiple
                value={config.breeds}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    breeds: Array.from(event.currentTarget.selectedOptions).map(
                      (option) => option.value,
                    ),
                  })
                }
              >
                {BREEDS.map((breed) => (
                  <option key={breed} value={breed}>
                    {t(`breed.${breed}`)}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              {t("field.countStrategy")}
              <div className="segmented">
                <button
                  className={config.cat_count_strategy === "MatchDisplays" ? "active" : ""}
                  onClick={() =>
                    setConfig({ ...config, cat_count_strategy: "MatchDisplays" })
                  }
                  type="button"
                >
                  {t("count.match")}
                </button>
                <button
                  className={config.cat_count_strategy === "Fixed" ? "active" : ""}
                  onClick={() => setConfig({ ...config, cat_count_strategy: "Fixed" })}
                  type="button"
                >
                  {t("count.fixed")}
                </button>
              </div>
              <small>
                {config.cat_count_strategy === "MatchDisplays"
                  ? t("hint.matchDisplays")
                  : t("hint.fixedOne")}
              </small>
            </label>
            {config.cat_count_strategy === "Fixed" && (
              <label className="field">
                {t("field.catCount")}
                <input
                  type="range"
                  min="1"
                  max="5"
                  value={config.cat_count}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      cat_count: Number(event.currentTarget.value),
                    })
                  }
                />
                <span className="range-value">{config.cat_count}</span>
              </label>
            )}
          </Panel>

          <Panel title={t("panel.quality")}>
            <div className="quality-grid">
              <label className="field">
                {t("field.minResolution")}
                <ResolutionInputs
                  height={config.image_quality.min_height}
                  onChange={(width, height) =>
                    setConfig({
                      ...config,
                      image_quality: {
                        ...config.image_quality,
                        min_width: width,
                        min_height: height,
                      },
                    })
                  }
                  width={config.image_quality.min_width}
                />
              </label>
              <label className="field">
                {t("field.preferredResolution")}
                <ResolutionInputs
                  height={config.image_quality.preferred_height}
                  onChange={(width, height) =>
                    setConfig({
                      ...config,
                      image_quality: {
                        ...config.image_quality,
                        preferred_width: width,
                        preferred_height: height,
                      },
                    })
                  }
                  width={config.image_quality.preferred_width}
                />
              </label>
            </div>
            <label className="switch">
              <span>
                <strong>{t("field.lowResFallback")}</strong>
                <small>{t("hint.lowResFallback")}</small>
              </span>
              <input
                type="checkbox"
                checked={config.image_quality.allow_low_resolution_fallback}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    image_quality: {
                      ...config.image_quality,
                      allow_low_resolution_fallback: event.currentTarget.checked,
                    },
                  })
                }
              />
            </label>
          </Panel>

          <Panel title={t("panel.mood")}>
            <div className="chips">
              {IMAGE_TYPES.map((type) => (
                <button
                  className={config.image_types.includes(type) ? "chip active" : "chip"}
                  key={type}
                  onClick={() =>
                    setConfig({
                      ...config,
                      image_types: toggleArray(config.image_types, type),
                    })
                  }
                  type="button"
                >
                  {t(`image.${type}`)}
                </button>
              ))}
            </div>
          </Panel>

          <Panel title={t("panel.interactions")}>
            {Object.entries(config.interactions).map(([key, enabled]) => (
              <label className="switch" key={key}>
                <span>
                  <strong>{t(`interaction.${key}`)}</strong>
                  <small>{t(`hint.${key}`)}</small>
                </span>
                <input
                  type="checkbox"
                  checked={enabled}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      interactions: {
                        ...config.interactions,
                        [key]: event.currentTarget.checked,
                      },
                    })
                  }
                />
              </label>
            ))}
          </Panel>

          <Panel title={t("panel.schedule")}>
            <div className="segmented">
              {["Daily", "EveryHours", "OnLogin", "ManualOnly"].map((kind) => (
                <button
                  className={scheduleKind === kind ? "active" : ""}
                  key={kind}
                  onClick={() => setConfig({ ...config, schedule: scheduleForKind(kind) })}
                  type="button"
                >
                  {scheduleLabel(kind, t)}
                </button>
              ))}
            </div>
            {scheduleKind === "Daily" && (
              <label className="field">
                {t("field.dailyTime")}
                <input
                  type="time"
                  value={dailyTime(config.schedule)}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      schedule: { Daily: { time: event.currentTarget.value } },
                    })
                  }
                />
              </label>
            )}
            {scheduleKind === "EveryHours" && (
              <label className="field">
                {t("field.everyHours")}
                <input
                  type="number"
                  min="1"
                  max="24"
                  value={intervalHours(config.schedule)}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      schedule: {
                        EveryHours: { hours: Number(event.currentTarget.value) },
                      },
                    })
                  }
                />
              </label>
            )}
          </Panel>

          <Panel title={t("panel.sources")}>
            <label className="switch">
              <span>
                <strong>Wikimedia Commons</strong>
                <small>{t("source.wikimedia")}</small>
              </span>
              <input
                type="checkbox"
                checked={config.sources.wikimedia_commons}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    sources: {
                      ...config.sources,
                      wikimedia_commons: event.currentTarget.checked,
                    },
                  })
                }
              />
            </label>
            <label className="switch">
              <span>
                <strong>CATAAS</strong>
                <small>{t("source.cataas")}</small>
              </span>
              <input
                type="checkbox"
                checked={config.sources.cataas}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    sources: { ...config.sources, cataas: event.currentTarget.checked },
                  })
                }
              />
            </label>
            <label className="switch">
              <span>
                <strong>TheCatAPI</strong>
                <small>{t("source.theCatApi")}</small>
              </span>
              <input
                type="checkbox"
                checked={config.sources.the_cat_api}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    sources: {
                      ...config.sources,
                      the_cat_api: event.currentTarget.checked,
                    },
                  })
                }
              />
            </label>
            <label className="field">
              {t("field.pexelsKey")}
              <div className="inline-field">
                <input
                  placeholder="Pexels API Key"
                  type="password"
                  value={config.sources.pexels_api_key ?? ""}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      sources: {
                        ...config.sources,
                        pexels_api_key: event.currentTarget.value || null,
                      },
                    })
                  }
                />
                <button
                  type="button"
                  onClick={() =>
                    window.open("https://www.pexels.com/zh-cn/api/key/", "_blank", "noopener")
                  }
                >
                  {t("source.getPexelsKey")}
                </button>
              </div>
              <small>{t("source.pexels")}</small>
            </label>
            <label className="field">
              {t("field.pixabayKey")}
              <input
                placeholder="Pixabay API Key"
                type="password"
                value={config.sources.pixabay_api_key ?? ""}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    sources: {
                      ...config.sources,
                      pixabay_api_key: event.currentTarget.value || null,
                    },
                  })
                }
              />
              <small>{t("source.pixabay")}</small>
            </label>
            <label className="field">
              {t("field.magnificKey")}
              <input
                placeholder="Magnific API Key"
                type="password"
                value={config.sources.magnific_api_key ?? ""}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    sources: {
                      ...config.sources,
                      magnific_api_key: event.currentTarget.value || null,
                    },
                  })
                }
              />
              <small>{t("source.magnific")}</small>
            </label>
            <div className="inline-field">
              <input
                placeholder={t("source.localPlaceholder")}
                value={localDirInput}
                onChange={(event) => setLocalDirInput(event.currentTarget.value)}
              />
              <button
                type="button"
                onClick={() => {
                  if (!localDirInput.trim()) return;
                  setConfig({
                    ...config,
                    sources: {
                      ...config.sources,
                      local_dirs: [...config.sources.local_dirs, localDirInput.trim()],
                    },
                  });
                  setLocalDirInput("");
                }}
              >
                {t("source.add")}
              </button>
            </div>
            <div className="source-list">
              {config.sources.local_dirs.map((dir) => (
                <button
                  key={dir}
                  type="button"
                  onClick={() =>
                    setConfig({
                      ...config,
                      sources: {
                        ...config.sources,
                        local_dirs: config.sources.local_dirs.filter((item) => item !== dir),
                      },
                    })
                  }
                >
                  {dir}
                </button>
              ))}
            </div>
          </Panel>

          <Panel title={t("panel.platform")}>
            <label className="field">
              {t("field.mode")}
              <select
                value={config.platform_mode}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    platform_mode: event.currentTarget.value as PlatformMode,
                  })
                }
              >
                <option value="Automatic">{t("mode.automatic")}</option>
                <option value="StaticOnly">{t("mode.static")}</option>
                <option value="InteractionBeta">{t("mode.beta")}</option>
              </select>
            </label>
            <label className="switch">
              <span>
                <strong>{t("field.launch")}</strong>
                <small>{t("hint.launch")}</small>
              </span>
              <input
                type="checkbox"
                checked={config.launch_at_login}
                onChange={(event) =>
                  setConfig({ ...config, launch_at_login: event.currentTarget.checked })
                }
              />
            </label>
          </Panel>
        </section>
      </section>
    </main>
  );
}

function Panel({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="panel">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function ResolutionInputs({
  width,
  height,
  onChange,
}: {
  width: number;
  height: number;
  onChange: (width: number, height: number) => void;
}) {
  return (
    <div className="resolution-inputs">
      <input
        min="1920"
        type="number"
        value={width}
        onChange={(event) => onChange(Number(event.currentTarget.value), height)}
      />
      <span>x</span>
      <input
        min="1080"
        type="number"
        value={height}
        onChange={(event) => onChange(width, Number(event.currentTarget.value))}
      />
    </div>
  );
}

function toggleArray<T>(items: T[], item: T): T[] {
  return items.includes(item) ? items.filter((value) => value !== item) : [...items, item];
}

function getScheduleKind(schedule: ScheduleConfig): string {
  if (typeof schedule === "string") return schedule;
  if ("Daily" in schedule) return "Daily";
  return "EveryHours";
}

function dailyTime(schedule: ScheduleConfig): string {
  return typeof schedule === "object" && "Daily" in schedule ? schedule.Daily.time : "09:00";
}

function intervalHours(schedule: ScheduleConfig): number {
  return typeof schedule === "object" && "EveryHours" in schedule
    ? schedule.EveryHours.hours
    : 4;
}

function scheduleForKind(kind: string): ScheduleConfig {
  switch (kind) {
    case "OnLogin":
      return "OnLogin";
    case "ManualOnly":
      return "ManualOnly";
    case "EveryHours":
      return { EveryHours: { hours: 4 } };
    default:
      return { Daily: { time: "09:00" } };
  }
}

function scheduleLabel(kind: string, t: (key: string) => string): string {
  const labels: Record<string, string> = {
    Daily: "schedule.daily",
    EveryHours: "schedule.everyHours",
    OnLogin: "schedule.onLogin",
    ManualOnly: "schedule.manual",
  };
  return t(labels[kind] ?? kind);
}

function catAssignments(displayCount: number, config: AppConfig): number[] {
  const count = Math.max(displayCount, 1);
  if (config.cat_count_strategy === "MatchDisplays") {
    return Array.from({ length: count }, (_, index) => index);
  }
  const uniqueCount = Math.max(config.cat_count, 1);
  return Array.from({ length: count }, (_, index) =>
    uniqueCount === 1 ? 0 : index % uniqueCount,
  );
}

function resolveLocale(language: LanguagePreference): Locale {
  switch (language) {
    case "English":
      return "en";
    case "SimplifiedChinese":
      return "zh-Hans";
    case "TraditionalChinese":
      return "zh-Hant";
    case "Japanese":
      return "ja";
    case "Korean":
      return "ko";
    default:
      return detectLocale();
  }
}

function detectLocale(): Locale {
  const languages = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const language of languages) {
    const normalized = language.toLowerCase();
    if (normalized.startsWith("zh-hant") || /zh-(tw|hk|mo)/.test(normalized)) {
      return "zh-Hant";
    }
    if (normalized.startsWith("zh")) return "zh-Hans";
    if (normalized.startsWith("ja")) return "ja";
    if (normalized.startsWith("ko")) return "ko";
  }
  return "en";
}

function translator(locale: Locale) {
  return (key: string) => dictionary[locale][key] ?? dictionary.en[key] ?? key;
}

function format(template: string, values: Record<string, string | number>) {
  return Object.entries(values).reduce(
    (result, [key, value]) => result.replaceAll(`{${key}}`, String(value)),
    template,
  );
}

function hasTauriRuntime() {
  return Boolean(window.__TAURI_INTERNALS__);
}

async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  fallback?: T,
): Promise<T> {
  if (!hasTauriRuntime()) {
    if (fallback !== undefined) return fallback;
    throw new Error("Tauri runtime is not available");
  }
  return invoke<T>(command, args);
}

function fallbackDisplays(): DisplayGeometry[] {
  return [
    {
      x: 0,
      y: 0,
      width: window.screen?.width || 1920,
      height: window.screen?.height || 1080,
    },
  ];
}

function clientSlots(catCount: number): Rect[] {
  if (catCount < 1) return [];
  const count = Math.min(catCount, 5);
  const columns = count === 1 ? 1 : count <= 4 ? 2 : 3;
  const rows = Math.ceil(count / columns);
  const usable = { x: 420, y: 80, width: 1420, height: 840 };
  const cellWidth = usable.width / columns;
  const cellHeight = usable.height / rows;
  return Array.from({ length: count }, (_, index) => ({
    x: usable.x + (index % columns) * cellWidth + 24,
    y: usable.y + Math.floor(index / columns) * cellHeight + 24,
    width: Math.max(cellWidth - 48, 1),
    height: Math.max(cellHeight - 48, 1),
  }));
}

function mergeConfig(config: AppConfig): AppConfig {
  return {
    ...defaultConfig,
    ...config,
    image_quality: {
      ...defaultConfig.image_quality,
      ...(config.image_quality ?? {}),
    },
    interactions: {
      ...defaultConfig.interactions,
      ...(config.interactions ?? {}),
    },
    sources: {
      ...defaultConfig.sources,
      ...(config.sources ?? {}),
    },
  };
}

createRoot(document.getElementById("root")!).render(<App />);

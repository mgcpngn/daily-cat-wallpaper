import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
type AiImageProvider = "OpenAi" | "GoogleNanoBananaPro" | "QwenImage";
type PromptTemplate =
  | "DesktopLayer"
  | "TaskbarPeek"
  | "IconCompanion"
  | "WindowCorner"
  | "FloatingSticker";
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
    selected_gallery_image: string | null;
  };
  ai_generation: {
    provider: AiImageProvider;
    openai_api_key: string | null;
    google_api_key: string | null;
    qwen_api_key: string | null;
    openai_model: string;
    google_model: string;
    qwen_model: string;
    prompt_template: PromptTemplate;
    scene: string;
    count: number;
    transparent_cutout: boolean;
    auto_use_generated: boolean;
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

type WallpaperIssue =
  | "NotVirtualDesktopSized"
  | "BelowMinimumResolution"
  | "PlaceholderGeneratedArt"
  | "NonTransparentPetAsset"
  | "SubjectTouchesImageEdge"
  | "PreferenceMatchUnverified";

type WallpaperAnalysis = {
  wallpaper_path: string;
  width: number;
  height: number;
  virtual_width: number;
  virtual_height: number;
  fills_virtual_desktop: boolean;
  meets_resolution: boolean;
  uses_placeholder_art: boolean;
  transparent_pet_asset: boolean;
  likely_cropped: boolean;
  unverified_preference_match: boolean;
  issues: WallpaperIssue[];
  score: number;
};

type WallpaperResult = {
  path: string;
  analysis: WallpaperAnalysis;
};

type GalleryImage = {
  path: string;
  file_name: string;
  source: string;
  width: number;
  height: number;
  meets_quality: boolean;
  transparent: boolean;
  feedback_score: number;
  rejected: boolean;
  thumbnail_data_url: string;
};

type ImportImagePayload = {
  file_name: string;
  data_base64: string;
};

type LearningSummary = {
  liked: number;
  disliked: number;
  rejected_images: number;
  top_reasons: string[];
};

type ApiKeyValidation = {
  provider: AiImageProvider;
  model: string;
  valid: boolean;
  message: string;
};

type AiGenerationProgress = {
  stage: "idle" | "validating" | "requesting" | "saving" | "completed" | "failed";
  message: string;
  current: number;
  total: number;
};

type InteractionLayerPayload = {
  image_path: string;
  image_data_url: string;
  interactions: AppConfig["interactions"];
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    webkitAudioContext?: typeof AudioContext;
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
const PROMPT_TEMPLATES: PromptTemplate[] = [
  "DesktopLayer",
  "TaskbarPeek",
  "IconCompanion",
  "WindowCorner",
  "FloatingSticker",
];
const LANGUAGES: Array<{ value: LanguagePreference; labelKey: string }> = [
  { value: "Auto", labelKey: "language.auto" },
  { value: "English", labelKey: "language.en" },
  { value: "SimplifiedChinese", labelKey: "language.zhHans" },
  { value: "TraditionalChinese", labelKey: "language.zhHant" },
  { value: "Japanese", labelKey: "language.ja" },
  { value: "Korean", labelKey: "language.ko" },
];

const baseDefaultConfig: AppConfig = {
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
    selected_gallery_image: null,
  },
  ai_generation: {
    provider: "OpenAi",
    openai_api_key: null,
    google_api_key: null,
    qwen_api_key: null,
    openai_model: "gpt-image-1.5",
    google_model: "gemini-3-pro-image-preview",
    qwen_model: "qwen-image-2.0-pro",
    prompt_template: "DesktopLayer",
    scene: "sitting naturally on the desktop edge",
    count: 4,
    transparent_cutout: true,
    auto_use_generated: true,
  },
  platform_mode: "Automatic",
  launch_at_login: true,
};

const defaultConfig: AppConfig = withLocaleAiDefaults(baseDefaultConfig, detectLocale());

const dictionary: Record<Locale, Record<string, string>> = {
  en: {
    "app.name": "Daily Cat Wallpaper",
    "hero.title": "Cat-powered desktop control.",
    "hero.lead":
      "Choose language, cat sources, HD quality, monitor behavior, and interaction style. The Rust core takes over wallpaper refreshes.",
    "action.refresh": "Refresh now",
    "action.prefetch": "Cache HD pack",
    "action.generateAi": "Generate transparent cats",
    "action.validateAiKey": "Validate API key",
    "action.startInteraction": "Start interaction layer",
    "action.stopInteraction": "Stop interaction layer",
    "action.save": "Save preferences",
    "action.select": "Use",
    "action.delete": "Delete",
    "action.import": "Import cat images",
    "action.openGallery": "Open gallery folder",
    "action.reloadGallery": "Reload gallery",
    "action.like": "Like",
    "action.dislike": "Dislike",
    "status.label": "Status",
    "status.loading": "Loading configuration",
    "status.ready": "Ready",
    "status.preview": "Preview mode: open the Tauri app to change wallpaper.",
    "status.saving": "Saving preferences",
    "status.saved": "Preferences saved",
    "status.refreshing": "Refreshing wallpaper",
    "status.prefetching": "Caching HD cat pack",
    "status.prefetched": "Cached {count} HD cat images",
    "status.generatingAi": "Generating transparent cat cutouts",
    "status.generatedAi": "Generated {count} transparent cats",
    "status.validatingAiKey": "Validating AI API key",
    "status.savingAi": "Saving generated cat images",
    "status.aiKeyValid": "API key OK: {model}",
    "status.aiElapsed": "Elapsed {seconds}s",
    "status.aiDoneAlert": "Transparent cat generation finished: {count} images.",
    "status.interactionStarted": "Interaction layer started",
    "status.interactionStopped": "Interaction layer stopped",
    "status.galleryLoaded": "Gallery loaded",
    "status.importing": "Importing selected cat images",
    "status.imported": "Imported {count} cat images",
    "status.galleryOpened": "Gallery folder opened",
    "status.deleted": "Deleted image",
    "status.feedback": "Feedback saved, future selection adjusted",
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
    "panel.ai": "AI transparent cats",
    "panel.gallery": "Cat gallery",
    "panel.analysis": "Desktop effect check",
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
    "hint.lowResFallback": "Hard disabled: images below 2560x1440 are rejected before saving.",
    "field.aiProvider": "AI provider",
    "field.openaiModel": "OpenAI model",
    "field.googleModel": "Gemini model",
    "field.qwenModel": "Qwen model",
    "field.promptTemplate": "Prompt template",
    "field.openaiKey": "OpenAI API key",
    "field.googleKey": "Google API key",
    "field.qwenKey": "DashScope API key",
    "field.aiScene": "Desktop pet scene",
    "field.aiCount": "Images to generate",
    "field.transparentCutout": "Transparent PNG cutout",
    "field.autoUseGenerated": "Auto-use newest generated cat",
    "field.aiProgress": "Generation monitor",
    "hint.transparentCutout": "Creates a no-background cat asset that can be placed on the desktop.",
    "hint.aiKey": "Generation automatically validates the selected provider key before sending the image request.",
    "hint.aiProviderDefault": "Chinese locales default to Qwen Image 2.0 Pro. English locales default to OpenAI image generation, with Gemini available as an option.",
    "hint.promptTemplate": "Templates are professional transparent-layer prompts; breed, image mood, and your scene are still merged automatically.",
    "hint.aiScene": "Example: peeking from the taskbar, sitting beside icons, stretching on the desktop edge.",
    "gallery.path": "Fixed gallery: {path}",
    "gallery.empty": "No cat images in the managed gallery yet. Import HD transparent cats or generate AI transparent cats first.",
    "gallery.quality": "{width}x{height} / {source} / score {score}",
    "gallery.lowQuality": "Below 2K gate",
    "gallery.rejected": "Rejected by feedback",
    "gallery.transparent": "Transparent",
    "gallery.photo": "Photo/background",
    "gallery.selected": "Selected",
    "gallery.help": "Pick images yourself, then press Use. Low-resolution images are rejected during import.",
    "analysis.score": "Effect score {score}/100",
    "analysis.size": "Final {width}x{height}, desktop {virtualWidth}x{virtualHeight}",
    "analysis.clean": "No measurable issues found.",
    "issue.NotVirtualDesktopSized": "Final image does not fill the actual virtual desktop.",
    "issue.BelowMinimumResolution": "Final image is below the 2K quality gate.",
    "issue.PlaceholderGeneratedArt": "Using placeholder generated art instead of a real or AI cat asset.",
    "issue.NonTransparentPetAsset": "Source is not a transparent pet cutout.",
    "issue.SubjectTouchesImageEdge": "Transparent cat touches the image edge, likely cropped.",
    "issue.PreferenceMatchUnverified": "Breed or mood match is not independently verified.",
    "learning.summary": "{liked} liked, {disliked} disliked, {rejected} rejected",
    "feedback.reason": "Reason, e.g. wrong breed, too small, cropped",
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
    "prompt.DesktopLayer": "Desktop transparent layer",
    "prompt.TaskbarPeek": "Peek over taskbar",
    "prompt.IconCompanion": "Sit beside icons",
    "prompt.WindowCorner": "Lean by window corner",
    "prompt.FloatingSticker": "Floating sticker layer",
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
    "action.generateAi": "生成透明猫",
    "action.validateAiKey": "校验 API Key",
    "action.startInteraction": "启动互动层",
    "action.stopInteraction": "停止互动层",
    "action.save": "保存配置",
    "action.select": "使用",
    "action.delete": "删除",
    "action.import": "导入猫图",
    "action.openGallery": "打开图库目录",
    "action.reloadGallery": "刷新图库",
    "action.like": "喜欢",
    "action.dislike": "不喜欢",
    "status.label": "状态",
    "status.loading": "正在加载配置",
    "status.ready": "已就绪",
    "status.preview": "预览模式：请打开 Tauri 应用来真正更换壁纸。",
    "status.saving": "正在保存偏好",
    "status.saved": "偏好已保存",
    "status.refreshing": "正在刷新壁纸",
    "status.prefetching": "正在缓存高清猫图包",
    "status.prefetched": "已缓存 {count} 张高清猫图",
    "status.generatingAi": "正在生成透明猫抠图",
    "status.generatedAi": "已生成 {count} 张透明猫图",
    "status.validatingAiKey": "正在校验 AI API Key",
    "status.savingAi": "正在保存生成的猫图",
    "status.aiKeyValid": "API Key 可用：{model}",
    "status.aiElapsed": "已等待 {seconds} 秒",
    "status.aiDoneAlert": "透明猫生成完成：{count} 张。",
    "status.interactionStarted": "互动层已启动",
    "status.interactionStopped": "互动层已停止",
    "status.galleryLoaded": "图库已加载",
    "status.importing": "正在导入已选择猫图",
    "status.imported": "已导入 {count} 张猫图",
    "status.galleryOpened": "图库目录已打开",
    "status.deleted": "图片已删除",
    "status.feedback": "反馈已保存，后续选择已调整",
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
    "panel.ai": "AI 透明猫",
    "panel.gallery": "猫图图库",
    "panel.analysis": "桌面效果体检",
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
    "hint.lowResFallback": "已硬性关闭：低于 2560x1440 的图片不会保存。",
    "field.aiProvider": "AI 提供方",
    "field.openaiModel": "OpenAI 模型",
    "field.googleModel": "Gemini 模型",
    "field.qwenModel": "通义万相模型",
    "field.promptTemplate": "提示词模板",
    "field.openaiKey": "OpenAI API Key",
    "field.googleKey": "Google API Key",
    "field.qwenKey": "DashScope API Key",
    "field.aiScene": "桌面宠物场景",
    "field.aiCount": "生成数量",
    "field.transparentCutout": "透明 PNG 抠图",
    "field.autoUseGenerated": "自动使用最新生成猫图",
    "field.aiProgress": "生成过程监控",
    "hint.transparentCutout": "生成没有背景的猫咪素材，再放到桌面上。",
    "hint.aiKey": "生成前会自动校验当前选择的提供方 API Key，然后再发送图片生成请求。",
    "hint.aiProviderDefault": "中文环境默认使用通义万相 Qwen Image 2.0 Pro；英文环境默认使用 OpenAI 图片模型，Gemini 可手动选择。",
    "hint.promptTemplate": "模板是专业透明图层提示词；品种、图片类型和你的场景仍会自动合并进去。",
    "hint.aiScene": "例如：从任务栏探头、坐在图标旁、趴在桌面边缘。",
    "gallery.path": "固定图库：{path}",
    "gallery.empty": "托管图库里还没有猫图。请先导入高清透明猫图，或生成 AI 透明猫。",
    "gallery.quality": "{width}x{height} / {source} / 评分 {score}",
    "gallery.lowQuality": "低于 2K 门槛",
    "gallery.rejected": "已被反馈拒用",
    "gallery.transparent": "透明图",
    "gallery.photo": "照片/带背景",
    "gallery.selected": "已选中",
    "gallery.help": "用户可以自己选图，导入后点“使用”。低分辨率图片会在导入时直接拒绝。",
    "analysis.score": "效果评分 {score}/100",
    "analysis.size": "成品 {width}x{height}，桌面 {virtualWidth}x{virtualHeight}",
    "analysis.clean": "未发现可量化问题。",
    "issue.NotVirtualDesktopSized": "成品图没有铺满实际虚拟桌面。",
    "issue.BelowMinimumResolution": "成品图低于 2K 质量门槛。",
    "issue.PlaceholderGeneratedArt": "使用了占位生成图，不是真实或 AI 猫素材。",
    "issue.NonTransparentPetAsset": "素材不是透明桌面宠物抠图。",
    "issue.SubjectTouchesImageEdge": "透明猫碰到图片边缘，可能被裁切。",
    "issue.PreferenceMatchUnverified": "品种或情绪匹配尚未被独立验证。",
    "learning.summary": "{liked} 个喜欢，{disliked} 个不喜欢，{rejected} 个拒用",
    "feedback.reason": "原因，例如品种不对、太小、被裁切",
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
    "prompt.DesktopLayer": "桌面透明图层",
    "prompt.TaskbarPeek": "从任务栏探头",
    "prompt.IconCompanion": "坐在图标旁",
    "prompt.WindowCorner": "靠在窗口角落",
    "prompt.FloatingSticker": "漂浮贴纸图层",
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
    "action.generateAi": "生成透明貓",
    "action.validateAiKey": "校驗 API Key",
    "action.startInteraction": "啟動互動層",
    "action.stopInteraction": "停止互動層",
    "action.save": "儲存設定",
    "action.select": "使用",
    "action.delete": "刪除",
    "action.import": "匯入貓圖",
    "action.openGallery": "開啟圖庫目錄",
    "action.reloadGallery": "重新載入圖庫",
    "action.like": "喜歡",
    "action.dislike": "不喜歡",
    "status.label": "狀態",
    "status.loading": "正在載入設定",
    "status.ready": "已就緒",
    "status.preview": "預覽模式：請開啟 Tauri 應用程式來真正更換桌布。",
    "status.saving": "正在儲存偏好",
    "status.saved": "偏好已儲存",
    "status.refreshing": "正在刷新桌布",
    "status.prefetching": "正在快取高清貓圖包",
    "status.prefetched": "已快取 {count} 張高清貓圖",
    "status.generatingAi": "正在生成透明貓去背圖",
    "status.generatedAi": "已生成 {count} 張透明貓圖",
    "status.validatingAiKey": "正在校驗 AI API Key",
    "status.savingAi": "正在儲存生成的貓圖",
    "status.aiKeyValid": "API Key 可用：{model}",
    "status.aiElapsed": "已等待 {seconds} 秒",
    "status.aiDoneAlert": "透明貓生成完成：{count} 張。",
    "status.interactionStarted": "互動層已啟動",
    "status.interactionStopped": "互動層已停止",
    "status.galleryLoaded": "圖庫已載入",
    "status.importing": "正在匯入已選貓圖",
    "status.imported": "已匯入 {count} 張貓圖",
    "status.galleryOpened": "圖庫目錄已開啟",
    "status.deleted": "圖片已刪除",
    "status.feedback": "回饋已儲存，後續選擇已調整",
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
    "panel.ai": "AI 透明貓",
    "panel.gallery": "貓圖圖庫",
    "panel.analysis": "桌面效果檢查",
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
    "hint.lowResFallback": "已硬性關閉：低於 2560x1440 的圖片不會儲存。",
    "field.aiProvider": "AI 提供方",
    "field.openaiModel": "OpenAI 模型",
    "field.googleModel": "Gemini 模型",
    "field.qwenModel": "通義萬相模型",
    "field.promptTemplate": "提示詞範本",
    "field.openaiKey": "OpenAI API Key",
    "field.googleKey": "Google API Key",
    "field.qwenKey": "DashScope API Key",
    "field.aiScene": "桌面寵物場景",
    "field.aiCount": "生成數量",
    "field.transparentCutout": "透明 PNG 去背",
    "field.autoUseGenerated": "自動使用最新生成貓圖",
    "field.aiProgress": "生成過程監控",
    "hint.transparentCutout": "生成無背景貓咪素材，再放到桌面上。",
    "hint.aiKey": "生成前會自動校驗目前選擇的提供方 API Key，然後再送出圖片生成請求。",
    "hint.aiProviderDefault": "中文環境預設使用通義萬相 Qwen Image 2.0 Pro；英文環境預設使用 OpenAI 圖片模型，Gemini 可手動選擇。",
    "hint.promptTemplate": "範本是專業透明圖層提示詞；品種、圖片類型和你的場景仍會自動合併。",
    "hint.aiScene": "例如：從工作列探頭、坐在圖示旁、趴在桌面邊緣。",
    "gallery.path": "固定圖庫：{path}",
    "gallery.empty": "受管理圖庫裡還沒有貓圖。請先匯入高清透明貓圖，或生成 AI 透明貓。",
    "gallery.quality": "{width}x{height} / {source} / 評分 {score}",
    "gallery.lowQuality": "低於 2K 門檻",
    "gallery.rejected": "已被回饋拒用",
    "gallery.transparent": "透明圖",
    "gallery.photo": "照片/帶背景",
    "gallery.selected": "已選取",
    "gallery.help": "使用者可以自己選圖，匯入後按「使用」。低解析度圖片會在匯入時直接拒絕。",
    "analysis.score": "效果評分 {score}/100",
    "analysis.size": "成品 {width}x{height}，桌面 {virtualWidth}x{virtualHeight}",
    "analysis.clean": "未發現可量化問題。",
    "issue.NotVirtualDesktopSized": "成品圖沒有鋪滿實際虛擬桌面。",
    "issue.BelowMinimumResolution": "成品圖低於 2K 品質門檻。",
    "issue.PlaceholderGeneratedArt": "使用了佔位生成圖，不是真實或 AI 貓素材。",
    "issue.NonTransparentPetAsset": "素材不是透明桌面寵物去背圖。",
    "issue.SubjectTouchesImageEdge": "透明貓碰到圖片邊緣，可能被裁切。",
    "issue.PreferenceMatchUnverified": "品種或情緒匹配尚未被獨立驗證。",
    "learning.summary": "{liked} 個喜歡，{disliked} 個不喜歡，{rejected} 個拒用",
    "feedback.reason": "原因，例如品種不對、太小、被裁切",
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
    "prompt.DesktopLayer": "桌面透明圖層",
    "prompt.TaskbarPeek": "從工作列探頭",
    "prompt.IconCompanion": "坐在圖示旁",
    "prompt.WindowCorner": "靠在視窗角落",
    "prompt.FloatingSticker": "漂浮貼紙圖層",
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
    "action.generateAi": "透明猫を生成",
    "action.validateAiKey": "API キーを検証",
    "action.startInteraction": "インタラクション層を開始",
    "action.stopInteraction": "インタラクション層を停止",
    "action.save": "設定を保存",
    "action.select": "使う",
    "action.delete": "削除",
    "action.import": "猫画像を読み込む",
    "action.openGallery": "ギャラリーを開く",
    "action.reloadGallery": "ギャラリーを再読み込み",
    "action.like": "好き",
    "action.dislike": "好きではない",
    "status.label": "状態",
    "status.loading": "設定を読み込み中",
    "status.ready": "準備完了",
    "status.preview": "プレビューモード: 壁紙変更には Tauri アプリを開いてください。",
    "status.saving": "設定を保存中",
    "status.saved": "設定を保存しました",
    "status.refreshing": "壁紙を更新中",
    "status.prefetching": "HD 猫画像パックを保存中",
    "status.prefetched": "{count} 枚の HD 猫画像を保存しました",
    "status.generatingAi": "透明猫カットアウトを生成中",
    "status.generatedAi": "{count} 枚の透明猫を生成しました",
    "status.validatingAiKey": "AI API キーを検証中",
    "status.savingAi": "生成した猫画像を保存中",
    "status.aiKeyValid": "API キー OK: {model}",
    "status.aiElapsed": "経過 {seconds} 秒",
    "status.aiDoneAlert": "透明猫の生成が完了しました: {count} 枚。",
    "status.interactionStarted": "インタラクション層を開始しました",
    "status.interactionStopped": "インタラクション層を停止しました",
    "status.galleryLoaded": "ギャラリーを読み込みました",
    "status.importing": "選択した猫画像を読み込み中",
    "status.imported": "{count} 枚の猫画像を読み込みました",
    "status.galleryOpened": "ギャラリーフォルダーを開きました",
    "status.deleted": "画像を削除しました",
    "status.feedback": "フィードバックを保存し、次回選択を調整しました",
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
    "panel.ai": "AI 透明猫",
    "panel.gallery": "猫ギャラリー",
    "panel.analysis": "デスクトップ効果チェック",
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
    "hint.lowResFallback": "固定で無効: 2560x1440 未満の画像は保存しません。",
    "field.aiProvider": "AI プロバイダー",
    "field.openaiModel": "OpenAI モデル",
    "field.googleModel": "Gemini モデル",
    "field.qwenModel": "Qwen モデル",
    "field.promptTemplate": "プロンプトテンプレート",
    "field.openaiKey": "OpenAI API キー",
    "field.googleKey": "Google API キー",
    "field.qwenKey": "DashScope API キー",
    "field.aiScene": "デスクトップペット場面",
    "field.aiCount": "生成枚数",
    "field.transparentCutout": "透明 PNG カットアウト",
    "field.autoUseGenerated": "最新生成猫を自動使用",
    "field.aiProgress": "生成モニター",
    "hint.transparentCutout": "背景なしの猫素材を作り、デスクトップに配置します。",
    "hint.aiKey": "生成前に選択したプロバイダーの API キーを自動検証し、その後に画像生成リクエストを送ります。",
    "hint.aiProviderDefault": "中国語環境では Qwen Image 2.0 Pro、英語環境では OpenAI 画像モデルを既定にし、Gemini も選択できます。",
    "hint.promptTemplate": "テンプレートは透明レイヤー向けの専門プロンプトです。品種、画像ムード、場面は自動で結合されます。",
    "hint.aiScene": "例: タスクバーから覗く、アイコン横に座る、デスクトップ端で伸びる。",
    "gallery.path": "固定ギャラリー: {path}",
    "gallery.empty": "管理ギャラリーに猫画像はまだありません。HD の透明猫画像を読み込むか、AI 透明猫を生成してください。",
    "gallery.quality": "{width}x{height} / {source} / score {score}",
    "gallery.lowQuality": "2K 基準未満",
    "gallery.rejected": "フィードバックで拒否済み",
    "gallery.transparent": "透明",
    "gallery.photo": "写真/背景あり",
    "gallery.selected": "選択中",
    "gallery.help": "ユーザーが画像を選び、読み込み後に「使う」を押せます。低解像度画像は読み込み時に拒否されます。",
    "analysis.score": "効果スコア {score}/100",
    "analysis.size": "出力 {width}x{height}、デスクトップ {virtualWidth}x{virtualHeight}",
    "analysis.clean": "測定可能な問題はありません。",
    "issue.NotVirtualDesktopSized": "出力画像が実際の仮想デスクトップを満たしていません。",
    "issue.BelowMinimumResolution": "出力画像が 2K 品質基準未満です。",
    "issue.PlaceholderGeneratedArt": "実写または AI 猫素材ではなくプレースホルダー画像です。",
    "issue.NonTransparentPetAsset": "素材が透明なデスクトップペット切り抜きではありません。",
    "issue.SubjectTouchesImageEdge": "透明猫が画像端に触れており、切れている可能性があります。",
    "issue.PreferenceMatchUnverified": "品種やムードの一致は未検証です。",
    "learning.summary": "好き {liked}、好きではない {disliked}、拒否 {rejected}",
    "feedback.reason": "理由: 品種違い、小さい、切れている など",
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
    "prompt.DesktopLayer": "デスクトップ透明レイヤー",
    "prompt.TaskbarPeek": "タスクバーから覗く",
    "prompt.IconCompanion": "アイコン横に座る",
    "prompt.WindowCorner": "ウィンドウ角に寄る",
    "prompt.FloatingSticker": "浮遊ステッカーレイヤー",
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
    "action.generateAi": "투명 고양이 생성",
    "action.validateAiKey": "API 키 검증",
    "action.startInteraction": "상호작용 레이어 시작",
    "action.stopInteraction": "상호작용 레이어 중지",
    "action.save": "설정 저장",
    "action.select": "사용",
    "action.delete": "삭제",
    "action.import": "고양이 이미지 가져오기",
    "action.openGallery": "갤러리 폴더 열기",
    "action.reloadGallery": "갤러리 다시 불러오기",
    "action.like": "좋아요",
    "action.dislike": "싫어요",
    "status.label": "상태",
    "status.loading": "설정 불러오는 중",
    "status.ready": "준비됨",
    "status.preview": "미리보기 모드: 배경화면 변경은 Tauri 앱에서 가능합니다.",
    "status.saving": "설정 저장 중",
    "status.saved": "설정 저장됨",
    "status.refreshing": "배경화면 새로고침 중",
    "status.prefetching": "HD 고양이 이미지 팩 캐시 중",
    "status.prefetched": "HD 고양이 이미지 {count}장 캐시됨",
    "status.generatingAi": "투명 고양이 컷아웃 생성 중",
    "status.generatedAi": "투명 고양이 {count}장 생성됨",
    "status.validatingAiKey": "AI API 키 검증 중",
    "status.savingAi": "생성된 고양이 이미지 저장 중",
    "status.aiKeyValid": "API 키 정상: {model}",
    "status.aiElapsed": "경과 {seconds}초",
    "status.aiDoneAlert": "투명 고양이 생성 완료: {count}장.",
    "status.interactionStarted": "상호작용 레이어 시작됨",
    "status.interactionStopped": "상호작용 레이어 중지됨",
    "status.galleryLoaded": "갤러리 로드됨",
    "status.importing": "선택한 고양이 이미지 가져오는 중",
    "status.imported": "고양이 이미지 {count}장 가져옴",
    "status.galleryOpened": "갤러리 폴더 열림",
    "status.deleted": "이미지 삭제됨",
    "status.feedback": "피드백 저장됨, 다음 선택에 반영됨",
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
    "panel.ai": "AI 투명 고양이",
    "panel.gallery": "고양이 갤러리",
    "panel.analysis": "데스크톱 효과 검사",
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
    "hint.lowResFallback": "항상 비활성: 2560x1440 미만 이미지는 저장하지 않습니다.",
    "field.aiProvider": "AI 제공자",
    "field.openaiModel": "OpenAI 모델",
    "field.googleModel": "Gemini 모델",
    "field.qwenModel": "Qwen 모델",
    "field.promptTemplate": "프롬프트 템플릿",
    "field.openaiKey": "OpenAI API 키",
    "field.googleKey": "Google API 키",
    "field.qwenKey": "DashScope API 키",
    "field.aiScene": "데스크톱 펫 장면",
    "field.aiCount": "생성 수",
    "field.transparentCutout": "투명 PNG 컷아웃",
    "field.autoUseGenerated": "최신 생성 고양이 자동 사용",
    "field.aiProgress": "생성 진행 모니터",
    "hint.transparentCutout": "배경 없는 고양이 소재를 만들어 데스크톱에 배치합니다.",
    "hint.aiKey": "생성 전 선택한 제공자의 API 키를 자동 검증한 뒤 이미지 생성 요청을 보냅니다.",
    "hint.aiProviderDefault": "중국어 환경은 Qwen Image 2.0 Pro를 기본값으로, 영어 환경은 OpenAI 이미지 모델을 기본값으로 사용하며 Gemini도 선택할 수 있습니다.",
    "hint.promptTemplate": "템플릿은 투명 레이어용 전문 프롬프트입니다. 품종, 이미지 분위기, 장면은 자동으로 합쳐집니다.",
    "hint.aiScene": "예: 작업 표시줄에서 고개 내밀기, 아이콘 옆에 앉기, 화면 가장자리에서 스트레칭.",
    "gallery.path": "고정 갤러리: {path}",
    "gallery.empty": "관리 갤러리에 아직 고양이 이미지가 없습니다. HD 투명 고양이 이미지를 가져오거나 AI 투명 고양이를 먼저 생성하세요.",
    "gallery.quality": "{width}x{height} / {source} / 점수 {score}",
    "gallery.lowQuality": "2K 기준 미만",
    "gallery.rejected": "피드백으로 거부됨",
    "gallery.transparent": "투명",
    "gallery.photo": "사진/배경 있음",
    "gallery.selected": "선택됨",
    "gallery.help": "사용자가 직접 이미지를 고른 뒤 가져오고, 사용 버튼을 누를 수 있습니다. 저해상도 이미지는 가져올 때 거부됩니다.",
    "analysis.score": "효과 점수 {score}/100",
    "analysis.size": "결과 {width}x{height}, 데스크톱 {virtualWidth}x{virtualHeight}",
    "analysis.clean": "측정 가능한 문제가 없습니다.",
    "issue.NotVirtualDesktopSized": "결과 이미지가 실제 가상 데스크톱을 채우지 않습니다.",
    "issue.BelowMinimumResolution": "결과 이미지가 2K 품질 기준 미만입니다.",
    "issue.PlaceholderGeneratedArt": "실제 또는 AI 고양이 소재가 아닌 임시 생성 그림입니다.",
    "issue.NonTransparentPetAsset": "소재가 투명 데스크톱 펫 컷아웃이 아닙니다.",
    "issue.SubjectTouchesImageEdge": "투명 고양이가 이미지 가장자리에 닿아 잘렸을 수 있습니다.",
    "issue.PreferenceMatchUnverified": "품종 또는 분위기 일치가 아직 검증되지 않았습니다.",
    "learning.summary": "좋아요 {liked}, 싫어요 {disliked}, 거부 {rejected}",
    "feedback.reason": "이유: 품종 불일치, 너무 작음, 잘림 등",
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
    "prompt.DesktopLayer": "데스크톱 투명 레이어",
    "prompt.TaskbarPeek": "작업 표시줄 위로 보기",
    "prompt.IconCompanion": "아이콘 옆에 앉기",
    "prompt.WindowCorner": "창 모서리에 기대기",
    "prompt.FloatingSticker": "플로팅 스티커 레이어",
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
  const [galleryPath, setGalleryPath] = useState("");
  const [galleryImages, setGalleryImages] = useState<GalleryImage[]>([]);
  const [latestAnalysis, setLatestAnalysis] = useState<WallpaperAnalysis | null>(null);
  const [learning, setLearning] = useState<LearningSummary>({
    liked: 0,
    disliked: 0,
    rejected_images: 0,
    top_reasons: [],
  });
  const [feedbackReason, setFeedbackReason] = useState("");
  const [aiBusy, setAiBusy] = useState(false);
  const [aiElapsed, setAiElapsed] = useState(0);
  const [aiKeyStatus, setAiKeyStatus] = useState("");
  const [aiProgress, setAiProgress] = useState<AiGenerationProgress>({
    stage: "idle",
    message: "",
    current: 0,
    total: 0,
  });
  const fileInputRef = useRef<HTMLInputElement | null>(null);
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
      safeInvoke<string>("gallery_location", undefined, ""),
      safeInvoke<GalleryImage[]>("list_gallery_images", undefined, []),
      safeInvoke<LearningSummary>("learning_summary", undefined, {
        liked: 0,
        disliked: 0,
        rejected_images: 0,
        top_reasons: [],
      }),
    ])
      .then(
        ([
          loadedConfig,
          loadedCapabilities,
          loadedDisplays,
          loadedGalleryPath,
          loadedGalleryImages,
          loadedLearning,
        ]) => {
        setConfig(mergeConfig(loadedConfig));
        setCapabilities(loadedCapabilities);
        setDisplays(loadedDisplays.length ? loadedDisplays : fallbackDisplays());
        setGalleryPath(loadedGalleryPath);
        setGalleryImages(loadedGalleryImages);
        setLearning(loadedLearning);
        setStatus(hasTauriRuntime() ? "status.ready" : "status.preview");
      },
      )
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

  useEffect(() => {
    if (!hasTauriRuntime()) return;
    let cancelled = false;
    let unlistenHandler: (() => void) | undefined;
    listen<AiGenerationProgress>("ai-generation-progress", (event) => {
      if (cancelled) return;
      const message = aiProgressMessage(event.payload, t);
      setAiProgress({ ...event.payload, message });
      setStatus(message);
      if (event.payload.stage === "completed" || event.payload.stage === "failed") {
        setAiBusy(false);
      }
    }).then((unlisten) => {
      unlistenHandler = unlisten;
      if (cancelled) {
        unlisten();
      }
    });
    return () => {
      cancelled = true;
      unlistenHandler?.();
    };
  }, [t]);

  useEffect(() => {
    if (!aiBusy) return;
    setAiElapsed(0);
    const timer = window.setInterval(() => {
      setAiElapsed((seconds) => seconds + 1);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [aiBusy]);

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
      const result = await safeInvoke<WallpaperResult | null>("refresh_wallpaper", undefined, null);
      if (!result) {
        setStatus("status.preview");
        return;
      }
      setLatestAnalysis(result.analysis);
      setStatus(
        result.path ? format(t("status.wallpaperSet"), { path: result.path }) : "status.preview",
      );
      await loadGallery();
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

  async function loadGallery() {
    const [path, images, summary] = await Promise.all([
      safeInvoke<string>("gallery_location", undefined, ""),
      safeInvoke<GalleryImage[]>("list_gallery_images", undefined, []),
      safeInvoke<LearningSummary>("learning_summary", undefined, learning),
    ]);
    setGalleryPath(path);
    setGalleryImages(images);
    setLearning(summary);
  }

  async function generateAiCats() {
    setAiBusy(true);
    setAiElapsed(0);
    setAiProgress({
      stage: "validating",
      message: t("status.validatingAiKey"),
      current: 0,
      total: config.ai_generation.count,
    });
    setStatus("status.validatingAiKey");
    try {
      const saved = await safeInvoke<AppConfig>("save_config", { config }, config);
      setConfig(mergeConfig(saved));
      const validation = await safeInvoke<ApiKeyValidation>("validate_ai_api_key");
      setAiKeyStatus(format(t("status.aiKeyValid"), { model: validation.model }));
      setStatus(format(t("status.aiKeyValid"), { model: validation.model }));
      setAiProgress({
        stage: "requesting",
        message: t("status.generatingAi"),
        current: 0,
        total: config.ai_generation.count,
      });
      const images = await safeInvoke<GalleryImage[]>(
        "generate_ai_cat_images",
        { count: config.ai_generation.count },
        [],
      );
      setGalleryImages(images);
      setStatus(format(t("status.generatedAi"), { count: images.length }));
      setAiProgress({
        stage: "completed",
        message: format(t("status.generatedAi"), { count: images.length }),
        current: images.length,
        total: config.ai_generation.count,
      });
      await loadGallery();
      window.alert(format(t("status.aiDoneAlert"), { count: images.length }));
    } catch (error) {
      setStatus(String(error));
      setAiProgress({
        stage: "failed",
        message: String(error),
        current: 0,
        total: config.ai_generation.count,
      });
    } finally {
      setAiBusy(false);
    }
  }

  async function validateAiKey() {
    setStatus("status.validatingAiKey");
    setAiKeyStatus("");
    try {
      const saved = await safeInvoke<AppConfig>("save_config", { config }, config);
      setConfig(mergeConfig(saved));
      const validation = await safeInvoke<ApiKeyValidation>("validate_ai_api_key");
      const message = format(t("status.aiKeyValid"), { model: validation.model });
      setAiKeyStatus(message);
      setStatus(message);
    } catch (error) {
      setAiKeyStatus(String(error));
      setStatus(String(error));
    }
  }

  async function startInteractionLayer() {
    setStatus("status.saving");
    try {
      const saved = await safeInvoke<AppConfig>("save_config", { config }, config);
      setConfig(mergeConfig(saved));
      await safeInvoke<InteractionLayerPayload>("start_interaction_layer");
      setStatus("status.interactionStarted");
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function stopInteractionLayer() {
    try {
      await safeInvoke("stop_interaction_layer");
      setStatus("status.interactionStopped");
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function importSelectedFiles(event: React.ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.currentTarget.files ?? []);
    event.currentTarget.value = "";
    if (!files.length) return;
    setStatus("status.importing");
    try {
      const payloads: ImportImagePayload[] = await Promise.all(
        files.map(async (file) => ({
          file_name: file.name,
          data_base64: arrayBufferToBase64(await file.arrayBuffer()),
        })),
      );
      const imported = await safeInvoke<GalleryImage[]>(
        "import_gallery_images",
        { payloads },
        [],
      );
      setStatus(format(t("status.imported"), { count: imported.length }));
      await loadGallery();
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function openGalleryFolder() {
    try {
      await safeInvoke("open_gallery_folder");
      setStatus("status.galleryOpened");
      await loadGallery();
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function selectGalleryImage(path: string) {
    setStatus("status.refreshing");
    try {
      const result = await safeInvoke<WallpaperResult | null>(
        "select_gallery_image",
        { path },
        null,
      );
      if (result) {
        setLatestAnalysis(result.analysis);
        setStatus(format(t("status.wallpaperSet"), { path: result.path }));
        setConfig({
          ...config,
          sources: { ...config.sources, selected_gallery_image: path },
        });
      }
      await loadGallery();
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function deleteGalleryImage(path: string) {
    try {
      await safeInvoke("delete_gallery_image", { path });
      setStatus("status.deleted");
      await loadGallery();
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function sendFeedback(liked: boolean) {
    const path = latestAnalysis?.wallpaper_path ?? config.sources.selected_gallery_image;
    if (!path) return;
    try {
      const summary = await safeInvoke<LearningSummary>(
        "record_wallpaper_feedback",
        {
          input: {
            path,
            liked,
            reason: feedbackReason || null,
          },
        },
        learning,
      );
      setLearning(summary);
      setFeedbackReason("");
      setStatus("status.feedback");
      await loadGallery();
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
                  setConfig(
                    mergeConfig({
                      ...config,
                      language: event.currentTarget.value as LanguagePreference,
                    }),
                  )
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
                checked={false}
                disabled
                onChange={() =>
                  setConfig({
                    ...config,
                    image_quality: {
                      ...config.image_quality,
                      allow_low_resolution_fallback: false,
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
            <div className="interaction-actions">
              <button className="secondary" type="button" onClick={startInteractionLayer}>
                {t("action.startInteraction")}
              </button>
              <button type="button" onClick={stopInteractionLayer}>
                {t("action.stopInteraction")}
              </button>
            </div>
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

          <Panel title={t("panel.ai")}>
            <label className="field">
              {t("field.aiProvider")}
              <select
                value={config.ai_generation.provider}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      provider: event.currentTarget.value as AiImageProvider,
                    },
                  })
                }
              >
                <option value="OpenAi">OpenAI GPT Image 1.5</option>
                <option value="GoogleNanoBananaPro">Google Nano Banana Pro</option>
                <option value="QwenImage">Qwen Image 2.0 Pro (China)</option>
              </select>
              <small>{t("hint.aiProviderDefault")}</small>
            </label>
            <label className="field">
              {t("field.openaiModel")}
              <input
                value={config.ai_generation.openai_model}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      openai_model: event.currentTarget.value,
                    },
                  })
                }
              />
            </label>
            <label className="field">
              {t("field.googleModel")}
              <input
                value={config.ai_generation.google_model}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      google_model: event.currentTarget.value,
                    },
                  })
                }
              />
            </label>
            <label className="field">
              {t("field.qwenModel")}
              <input
                value={config.ai_generation.qwen_model}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      qwen_model: event.currentTarget.value,
                    },
                  })
                }
              />
            </label>
            <label className="field">
              {t("field.promptTemplate")}
              <select
                value={config.ai_generation.prompt_template}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      prompt_template: event.currentTarget.value as PromptTemplate,
                    },
                  })
                }
              >
                {PROMPT_TEMPLATES.map((template) => (
                  <option key={template} value={template}>
                    {t(`prompt.${template}`)}
                  </option>
                ))}
              </select>
              <small>{t("hint.promptTemplate")}</small>
            </label>
            <label className="field">
              {t("field.openaiKey")}
              <input
                placeholder="sk-..."
                type="password"
                value={config.ai_generation.openai_api_key ?? ""}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      openai_api_key: event.currentTarget.value || null,
                    },
                  })
                }
              />
            </label>
            <label className="field">
              {t("field.googleKey")}
              <input
                placeholder="AIza..."
                type="password"
                value={config.ai_generation.google_api_key ?? ""}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      google_api_key: event.currentTarget.value || null,
                    },
                  })
                }
              />
            </label>
            <label className="field">
              {t("field.qwenKey")}
              <input
                placeholder="sk-..."
                type="password"
                value={config.ai_generation.qwen_api_key ?? ""}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      qwen_api_key: event.currentTarget.value || null,
                    },
                  })
                }
              />
            </label>
            <button className="secondary panel-action" type="button" onClick={validateAiKey}>
              {t("action.validateAiKey")}
            </button>
            <p className="panel-note">{aiKeyStatus || t("hint.aiKey")}</p>
            <label className="field">
              {t("field.aiScene")}
              <input
                type="text"
                value={config.ai_generation.scene}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      scene: event.currentTarget.value,
                    },
                  })
                }
              />
              <small>{t("hint.aiScene")}</small>
            </label>
            <label className="field">
              {t("field.aiCount")}
              <input
                max="24"
                min="1"
                type="number"
                value={config.ai_generation.count}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      count: Number(event.currentTarget.value),
                    },
                  })
                }
              />
            </label>
            <label className="switch">
              <span>
                <strong>{t("field.transparentCutout")}</strong>
                <small>{t("hint.transparentCutout")}</small>
              </span>
              <input checked disabled type="checkbox" />
            </label>
            <label className="switch">
              <span>
                <strong>{t("field.autoUseGenerated")}</strong>
                <small>{t("hint.optional")}</small>
              </span>
              <input
                checked={config.ai_generation.auto_use_generated}
                type="checkbox"
                onChange={(event) =>
                  setConfig({
                    ...config,
                    ai_generation: {
                      ...config.ai_generation,
                      auto_use_generated: event.currentTarget.checked,
                    },
                  })
                }
              />
            </label>
            <div className="analysis-box">
              <strong>{t("field.aiProgress")}</strong>
              <small>{aiProgress.message || t("status.ready")}</small>
              {aiBusy && (
                <small>{format(t("status.aiElapsed"), { seconds: aiElapsed })}</small>
              )}
              {aiProgress.total > 0 && (
                <div className="progress-bar" aria-label={t("field.aiProgress")}>
                  <span
                    style={{
                      width: `${Math.min(
                        100,
                        Math.round((aiProgress.current / Math.max(aiProgress.total, 1)) * 100),
                      )}%`,
                    }}
                  />
                </div>
              )}
            </div>
            <button
              className="primary panel-action"
              disabled={aiBusy}
              type="button"
              onClick={generateAiCats}
            >
              {t("action.generateAi")}
            </button>
          </Panel>

          <Panel title={t("panel.gallery")}>
            <p className="panel-note">
              {format(t("gallery.path"), { path: galleryPath || "-" })}
            </p>
            <p className="panel-note">{t("gallery.help")}</p>
            <input
              ref={fileInputRef}
              accept="image/png,image/jpeg,image/webp"
              className="hidden-file-input"
              multiple
              onChange={importSelectedFiles}
              type="file"
            />
            <div className="gallery-actions">
              <button type="button" onClick={() => fileInputRef.current?.click()}>
                {t("action.import")}
              </button>
              <button type="button" onClick={openGalleryFolder}>
                {t("action.openGallery")}
              </button>
              <button type="button" onClick={loadGallery}>
                {t("action.reloadGallery")}
              </button>
            </div>
            <div className="gallery-grid">
              {galleryImages.length === 0 && <p className="panel-note">{t("gallery.empty")}</p>}
              {galleryImages.map((image) => (
                <article
                  className={[
                    "gallery-item",
                    image.rejected ? "rejected" : "",
                    config.sources.selected_gallery_image === image.path ? "selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  key={image.path}
                >
                  <img alt={image.file_name} src={image.thumbnail_data_url || imageSrc(image.path)} />
                  <strong>{image.file_name}</strong>
                  <small>
                    {format(t("gallery.quality"), {
                      width: image.width,
                      height: image.height,
                      source: image.source,
                      score: image.feedback_score,
                    })}
                  </small>
                  <div className="badges">
                    <span>{image.transparent ? t("gallery.transparent") : t("gallery.photo")}</span>
                    {config.sources.selected_gallery_image === image.path && (
                      <span>{t("gallery.selected")}</span>
                    )}
                    {!image.meets_quality && <span>{t("gallery.lowQuality")}</span>}
                    {image.rejected && <span>{t("gallery.rejected")}</span>}
                  </div>
                  <div className="mini-actions">
                    <button type="button" onClick={() => selectGalleryImage(image.path)}>
                      {t("action.select")}
                    </button>
                    <button type="button" onClick={() => deleteGalleryImage(image.path)}>
                      {t("action.delete")}
                    </button>
                  </div>
                </article>
              ))}
            </div>
          </Panel>

          <Panel title={t("panel.analysis")}>
            {latestAnalysis ? (
              <div className="analysis-box">
                <strong>{format(t("analysis.score"), { score: latestAnalysis.score })}</strong>
                <small>
                  {format(t("analysis.size"), {
                    width: latestAnalysis.width,
                    height: latestAnalysis.height,
                    virtualWidth: latestAnalysis.virtual_width,
                    virtualHeight: latestAnalysis.virtual_height,
                  })}
                </small>
                {latestAnalysis.issues.length ? (
                  <ul>
                    {latestAnalysis.issues.map((issue) => (
                      <li key={issue}>{t(`issue.${issue}`)}</li>
                    ))}
                  </ul>
                ) : (
                  <p>{t("analysis.clean")}</p>
                )}
              </div>
            ) : (
              <p className="panel-note">{t("analysis.clean")}</p>
            )}
            <p className="panel-note">
              {format(t("learning.summary"), {
                liked: learning.liked,
                disliked: learning.disliked,
                rejected: learning.rejected_images,
              })}
            </p>
            <div className="inline-field">
              <input
                placeholder={t("feedback.reason")}
                value={feedbackReason}
                onChange={(event) => setFeedbackReason(event.currentTarget.value)}
              />
              <button type="button" onClick={() => sendFeedback(false)}>
                {t("action.dislike")}
              </button>
            </div>
            <button className="secondary panel-action" type="button" onClick={() => sendFeedback(true)}>
              {t("action.like")}
            </button>
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

function CatOverlay() {
  const [payload, setPayload] = useState<InteractionLayerPayload | null>(null);
  const [near, setNear] = useState(false);
  const [paw, setPaw] = useState(false);
  const [bongo, setBongo] = useState(false);
  const stageRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    document.body.classList.add("overlay-mode");
    return () => document.body.classList.remove("overlay-mode");
  }, []);

  useEffect(() => {
    safeInvoke<InteractionLayerPayload>("get_interaction_layer")
      .then(setPayload)
      .catch(() => undefined);
    if (!hasTauriRuntime()) return;
    let cancelled = false;
    let unlistenHandler: (() => void) | undefined;
    listen<InteractionLayerPayload>("interaction-layer-update", (event) => {
      if (!cancelled) setPayload(event.payload);
    }).then((unlisten) => {
      unlistenHandler = unlisten;
      if (cancelled) unlisten();
    });
    return () => {
      cancelled = true;
      unlistenHandler?.();
    };
  }, []);

  useEffect(() => {
    stageRef.current?.focus();
  }, [payload]);

  function pulse(setter: React.Dispatch<React.SetStateAction<boolean>>) {
    setter(true);
    window.setTimeout(() => setter(false), 260);
  }

  function playSoftClick() {
    if (!payload?.interactions.sound) return;
    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (!AudioContextClass) return;
    const audio = new AudioContextClass();
    const oscillator = audio.createOscillator();
    const gain = audio.createGain();
    oscillator.type = "sine";
    oscillator.frequency.setValueAtTime(520, audio.currentTime);
    oscillator.frequency.exponentialRampToValueAtTime(720, audio.currentTime + 0.08);
    gain.gain.setValueAtTime(0.0001, audio.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.08, audio.currentTime + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.0001, audio.currentTime + 0.14);
    oscillator.connect(gain).connect(audio.destination);
    oscillator.start();
    oscillator.stop(audio.currentTime + 0.16);
  }

  if (!payload) {
    return <main className="cat-overlay-stage" />;
  }

  return (
    <main
      ref={stageRef}
      className={[
        "cat-overlay-stage",
        payload.interactions.breathing ? "breathing" : "",
        payload.interactions.mouse_proximity && near ? "near" : "",
        payload.interactions.click_paw && paw ? "paw" : "",
        payload.interactions.keyboard_bongo && bongo ? "bongo" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      tabIndex={0}
      onKeyDown={() => {
        if (!payload.interactions.keyboard_bongo) return;
        pulse(setBongo);
        playSoftClick();
      }}
      onMouseMove={(event) => {
        if (!payload.interactions.mouse_proximity) return;
        const rect = event.currentTarget.getBoundingClientRect();
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;
        const distance = Math.hypot(event.clientX - centerX, event.clientY - centerY);
        setNear(distance < Math.min(rect.width, rect.height) * 0.42);
      }}
      onMouseLeave={() => setNear(false)}
    >
      <button
        aria-label="cat interaction layer"
        className="cat-overlay-button"
        type="button"
        onClick={() => {
          if (payload.interactions.click_paw) pulse(setPaw);
          playSoftClick();
        }}
      >
        <img alt="" src={payload.image_data_url} />
      </button>
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
  if (typeof navigator === "undefined") return "en";
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

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 0x8000;
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
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
  const merged = {
    ...baseDefaultConfig,
    ...config,
    image_quality: {
      ...baseDefaultConfig.image_quality,
      ...(config.image_quality ?? {}),
    },
    interactions: {
      ...baseDefaultConfig.interactions,
      ...(config.interactions ?? {}),
    },
    sources: {
      ...baseDefaultConfig.sources,
      ...(config.sources ?? {}),
    },
    ai_generation: {
      ...baseDefaultConfig.ai_generation,
      ...(config.ai_generation ?? {}),
      transparent_cutout: true,
    },
  };
  return withLocaleAiDefaults(
    merged,
    resolveLocale(merged.language),
    !hasAnyAiProviderKey(config.ai_generation),
  );
}

function imageSrc(_path: string): string {
  return "";
}

function withLocaleAiDefaults(
  config: AppConfig,
  locale: Locale,
  applyProvider = true,
): AppConfig {
  const aiDefaults = localizedAiGenerationDefaults(locale);
  return {
    ...config,
    ai_generation: {
      ...aiDefaults,
      ...config.ai_generation,
      provider: applyProvider ? aiDefaults.provider : config.ai_generation.provider,
      openai_model: config.ai_generation.openai_model || aiDefaults.openai_model,
      google_model: config.ai_generation.google_model || aiDefaults.google_model,
      qwen_model: config.ai_generation.qwen_model || aiDefaults.qwen_model,
      transparent_cutout: true,
    },
  };
}

function localizedAiGenerationDefaults(locale: Locale): AppConfig["ai_generation"] {
  const englishDefaults = {
    ...baseDefaultConfig.ai_generation,
    provider: "OpenAi" as AiImageProvider,
    openai_model: "gpt-image-1.5",
    google_model: "gemini-3-pro-image-preview",
    qwen_model: "qwen-image-2.0-pro",
  };
  if (locale === "zh-Hans" || locale === "zh-Hant") {
    return {
      ...englishDefaults,
      provider: "QwenImage",
    };
  }
  return englishDefaults;
}

function hasAnyAiProviderKey(ai?: AppConfig["ai_generation"]): boolean {
  return Boolean(
    ai?.openai_api_key?.trim() || ai?.google_api_key?.trim() || ai?.qwen_api_key?.trim(),
  );
}

function aiProgressMessage(
  progress: AiGenerationProgress,
  t: (key: string) => string,
): string {
  if (progress.stage === "validating") return t("status.validatingAiKey");
  if (progress.stage === "requesting") return t("status.generatingAi");
  if (progress.stage === "saving") return t("status.savingAi");
  if (progress.stage === "completed") {
    return format(t("status.generatedAi"), { count: progress.current });
  }
  return progress.message;
}

function isOverlayPage(): boolean {
  return new URLSearchParams(window.location.search).has("overlay");
}

createRoot(document.getElementById("root")!).render(isOverlayPage() ? <CatOverlay /> : <App />);

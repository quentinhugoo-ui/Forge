import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const appSource = readFileSync(join(process.cwd(), "src", "renderer", "App.tsx"), "utf8");
const canvasSource = readFileSync(join(process.cwd(), "src", "renderer", "CanvasSurfacesSlice.tsx"), "utf8");
const rendererSource = readFileSync(join(process.cwd(), "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const storeSource = readFileSync(join(process.cwd(), "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");
const sidebarSource = readFileSync(join(process.cwd(), "src", "renderer", "SidebarSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(process.cwd(), "src", "renderer", "styles.css"), "utf8");
const preloadCjsSource = readFileSync(join(process.cwd(), "preload.cjs"), "utf8");
const brainSource = readFileSync(join(process.cwd(), "..", "..", "src", "brain.rs"), "utf8");

describe("LLM multimodal attachments", () => {
  it("normalizes local uploads into provider-native multimodal content", () => {
    expect(mainSource).toContain("openAiResponseContent");
    expect(mainSource).toContain('type: "input_image"');
    expect(mainSource).toContain('type: "input_file"');
    expect(mainSource).toContain("file_data: attachment.openAiFileDataUrl");
    expect(mainSource).toContain("const text = userTextWithAttachmentContext(userText, attachments)");
    expect(mainSource).toContain("attachments.flatMap((attachment) => attachment.visualSnapshots)");
    expect(mainSource).toContain('type: "image_url"');
    expect(mainSource).toContain("image_url: { url: snapshot.imageUrl }");
  });

  it("creates visual snapshots for images, videos, and 3D files", () => {
    expect(mainSource).toContain("captureImageSnapshot");
    expect(mainSource).toContain("analyzeVideoAttachment");
    expect(mainSource).toContain("model3dSummaryForFile");
    expect(mainSource).toContain('source: "image" | "video-frame" | "model3d-summary"');
    expect(mainSource).toContain("video frame extraction unavailable");
    expect(mainSource).toContain("3D model summary");
  });

  it("sends rich video metadata, multiple frames, and extracted subtitle cues to the LLM", () => {
    expect(mainSource).toContain("PANELS_CHAT_BOTTOM_MAX_VISUAL_SNAPSHOTS = 6");
    expect(mainSource).toContain("ProviderVideoMetadata");
    expect(mainSource).toContain("videoMetadataText");
    expect(mainSource).toContain("quality_label=");
    expect(mainSource).toContain("resolution=");
    expect(mainSource).toContain("duration=");
    expect(mainSource).toContain("snapshot_times=");
    expect(mainSource).toContain("video.textTracks");
    expect(mainSource).toContain("track.cues || track.activeCues");
    expect(mainSource).toContain("subtitles_extracted=");
  });

  it("extracts useful text from code and written documents", () => {
    expect(mainSource).toContain('".rs"');
    expect(mainSource).toContain('".py"');
    expect(mainSource).toContain('".docx"');
    expect(mainSource).toContain('".pptx"');
    expect(mainSource).toContain("officeTextPreviewForFile");
    expect(mainSource).toContain("xmlTextNodes");
    expect(mainSource).toContain("PANELS_CHAT_BOTTOM_MAX_TEXT_PREVIEW_BYTES");
  });

  it("carries recent session attachments into follow-up LLM turns", () => {
    expect(mainSource).toContain("const pendingUploadItems = composerUploadItemsForCommand(command)");
    expect(mainSource).toContain("composerUploadItemsForCommand(command)");
    expect(mainSource).toContain("function providerUploadItemsForCommand");
    expect(mainSource).toContain("message.attachments ?? []");
    expect(mainSource).toContain("composerUploadPreviewItems.get(preview.id)");
    expect(mainSource).toContain("const providerUploadItems = providerUploadItemsForCommand(pendingUploadItems)");
    expect(mainSource).toContain("providerAttachments = await providerAttachmentsFromUploads(providerUploadItems)");
    expect(mainSource).toContain("const currentAttachmentIds = new Set(pendingUploadItems.map((item) => item.id))");
    expect(mainSource).toContain("attachments: attachmentPreviews");
    expect(mainSource).toContain("await buildAssistantTranscriptMessage(draft, providerAttachments, message.id, moduleId, requestTranscriptWithUser)");
    expect(mainSource).toContain("providerAttachmentCache");
  });

  it("carries conversation memory and previous user intent into attachment-only provider turns", () => {
    expect(mainSource).toContain("function recentConversationWindow");
    expect(mainSource).toContain("PANELS_CHAT_BOTTOM_CONTEXT_TOKEN_BUDGET");
    expect(mainSource).toContain("PANELS_CHAT_BOTTOM_COMPACT_AT_TOKENS");
    expect(mainSource).toContain("PANELS_CHAT_BOTTOM_RECENT_CONTEXT_TOKENS");
    expect(mainSource).toContain("function conversationMemoryContext");
    expect(mainSource).toContain("function conversationContextPlan");
    expect(mainSource).toContain("function estimatedPromptTokens");
    expect(mainSource).toContain("BRAIN_REINJECTED_AFTER_COMPACTION");
    expect(mainSource).toContain("brainBootManifest()");
    expect(mainSource).toContain("Memoire compacte de la conversation anterieure:");
    expect(mainSource).toContain("PANELS_CHAT_BOTTOM_DOCUMENT_MEMORY_BYTES");
    expect(mainSource).toContain("function sessionDocumentMemoryContext");
    expect(mainSource).toContain("Registre des documents de la session:");
    expect(mainSource).toContain("created_or_returned_by_assistant=true");
    expect(mainSource).toContain("provided_by_user=true");
    expect(mainSource).toContain("function previousUserIntentForAttachmentTurn");
    expect(mainSource).toContain("function providerUserTextForTurn");
    expect(mainSource).toContain("TACHE ACTIVE OBLIGATOIRE:");
    expect(mainSource).toContain("Ne demande pas ce que l'utilisateur veut faire avec l'image ou le document.");
    expect(mainSource).toContain("L'utilisateur vient d'envoyer les pieces jointes pour repondre a sa demande precedente.");
    expect(mainSource).toContain("openAiResponseConversationInput(userText, attachments, userMessageId, transcript)");
    expect(mainSource).toContain("input: await openAiResponseConversationInput(userText, attachments, userMessageId, transcript)");
    expect(mainSource).toContain("Conversation recente:");
    expect(mainSource).toContain("const providerUserText = providerUserTextForTurn(userText, attachments, userMessageId, transcript)");
    expect(mainSource).toContain("function appendTranscriptMessageForActiveSession");
    expect(mainSource).toContain("const requestSessionId = activeSession.sessionId");
    expect(mainSource).toContain("const requestTranscriptBeforeSend = [...panelsChatBottomState.transcript]");
    expect(mainSource).toContain("buildAssistantTranscriptMessage(draft, providerAttachments, message.id, moduleId, requestTranscriptWithUser)");
  });

  it("executes /searcharchive_ locally as a Brain archive command", () => {
    expect(mainSource).toContain("parseSearchArchiveCodeAct(draft)");
    expect(mainSource).toContain("localSearchArchiveStatus(searchArchiveRequest)");
    expect(mainSource).toContain("forge:search-archive");
    expect(mainSource).toContain("archiveTranscriptMessage(activeSession, assistantMessage)");
  });

  it("boots every LLM session with Brain access before provider execution", () => {
    expect(mainSource).toContain("BRAIN_BOOT_MANIFEST v1");
    expect(mainSource).toContain("BRAIN_CODEACT_COMMANDS.join");
    expect(mainSource).toContain("ensureBrainBootTranscript");
    expect(mainSource).toContain("brainBootManifest()");
  });

  it("keeps Gmail CodeAct user-facing text normal and opens WebExplorer from results", () => {
    expect(mainSource).toContain("ecris ta propre phrase naturelle");
    expect(mainSource).toContain("L'application affiche automatiquement l'evenement");
    expect(mainSource).toContain("BRAIN_GMAIL_COM_COMMAND");
    expect(appSource).toContain('latestAssistant?.text.includes("GMAIL_RESULT")');
    expect(rendererSource).not.toContain("stripGmailCodeActLines");
    expect(rendererSource).not.toContain("J'ouvre Gmail dans Web Explorer.");
  });

  it("registers Airbnb as a Brain-backed module CodeAct", () => {
    expect(brainSource).toContain('pub const BRAIN_AIRBNB_COMMAND: &str = "/airbnb_"');
    expect(brainSource).toContain("BRAIN_CODEACT_ROUTING_RULES");
    expect(brainSource).toContain("trip/vacation intent with a destination");
    expect(brainSource).toContain("je veux partir en vacances au Japon en septembre");
    expect(brainSource).toContain("prefer /airbnb_ over /googleweb_");
    expect(brainSource).toContain("brain_airbnb_codeact_template()");
    expect(mainSource).toContain('if (moduleId === "airbnb")');
    expect(mainSource).toContain('Template Airbnb: ${BRAIN_AIRBNB_COMMAND}');
    expect(mainSource).toContain("codeact_routing_rules=${BRAIN_CODEACT_ROUTING_RULES}");
    expect(mainSource).toContain("BRAIN_CODEACT_ROUTING_RULES");
    expect(mainSource).toContain("N'utilise ${BRAIN_GOOGLEWEB_COMMAND} que pour une recherche web generique");
    expect(mainSource).toContain("N'utilise pas ${BRAIN_GOOGLEWEB_COMMAND} pour une action Airbnb.");
    expect(mainSource).toContain("executeAssistantAirbnbCodeAct");
    expect(mainSource).toContain("navigateNativeWebExplorerToAirbnb");
    expect(mainSource).toContain('parsed.hostname === "www.airbnb.com"');
    expect(appSource).toContain('latestAssistant?.text.includes("AIRBNB_RESULT")');
    expect(rendererSource).toContain('[BRAIN_AIRBNB_COMMAND, "Airbnb surface opened"]');
  });

  it("renders Gmail and Airbnb CodeAct events with their module logos", () => {
    expect(rendererSource).toContain('command === BRAIN_GMAIL_COMMAND || command === BRAIN_GMAIL_COM_COMMAND');
    expect(rendererSource).toContain('<ModuleLogo id="gmail" />');
    expect(rendererSource).toContain('if (command === BRAIN_AIRBNB_COMMAND) return <ModuleLogo id="airbnb" />');
    expect(stylesSource).toContain(".transcriptCodeActEvent__icon .sidebarModule__logo");
  });

  it("keeps CodeAct result metadata from creating duplicate Canvas events", () => {
    expect(rendererSource).toContain("if (isCodeActResultHeader(line))");
    expect(rendererSource).toContain("if (skippingCodeActMetadata) {\n      continue;\n    }\n    const event = codeActEventFromLine(line)");
    expect(rendererSource).not.toContain("const TRANSCRIPT_CODEACT_COMMANDS = [...BRAIN_CODEACT_COMMANDS, BRAIN_AIRBNB_COMMAND]");
  });

  it("mounts the native WebExplorer view instead of leaving the Google placeholder", () => {
    expect(mainSource).toContain("installMainProcessConsoleGuard");
    expect(mainSource).toContain("isBrokenPipeError");
    expect(mainSource).toContain("new BrowserView");
    expect(mainSource).not.toContain("function openGmailBrowserViewProbeWindow");
    expect(mainSource).toContain("function loadNativeWebExplorerTarget");
    expect(mainSource).toContain("nativeWebExplorerLoadedUrl");
    expect(mainSource).toContain("nativeWebExplorerBoundsKey");
    expect(mainSource).toContain("NATIVE_WEBEXPLORER_VIEWPORT_FADE_CSS");
    expect(mainSource).toContain("installNativeWebExplorerViewportFade(view)");
    expect(mainSource).toContain("view.webContents.insertCSS");
    expect(mainSource).toContain("configureNativeWebExplorerSession");
    expect(mainSource).toContain("setPermissionRequestHandler");
    expect(mainSource).toContain("webRequest.onBeforeRequest");
    expect(mainSource).toContain('parsed.hostname === "accounts.google.com"');
    expect(mainSource).toContain("gmailWebExplorerNavigationUrl(request)");
    expect(mainSource).toContain("nativeWebExplorerTargetUrl = navigationUrl");
    expect(mainSource).toContain("function isGmailMarketingLandingUrl");
    expect(mainSource).toContain("callback({ redirectURL: GMAIL_SIGN_IN_URL })");
    expect(mainSource).toContain('loadNativeWebExplorerTarget(view, "gmail-landing-redirect")');
    expect(mainSource).toContain("attachNativeWebExplorerView(owner, view)");
    expect(mainSource).toContain("owner.addBrowserView(view)");
    expect(mainSource).toContain("owner.setTopBrowserView(view)");
    expect(mainSource).not.toContain("nativeWebExplorerOwner?.setTopBrowserView(nativeWebExplorerView)");
    expect(mainSource).toContain("owner?.removeBrowserView(view)");
    expect(mainSource).toContain("view.setBackgroundColor(\"#ffffff\")");
    expect(mainSource).toContain("view.webContents.setUserAgent(CHATGPT_USER_AGENT)");
    expect(mainSource).toContain('loadNativeWebExplorerTarget(view, "show")');
    expect(mainSource).toContain('preload: join(shellRoot, "preload.cjs")');
    expect(preloadCjsSource).toContain("showNativeWebExplorer(bounds)");
    expect(preloadCjsSource).toContain('ipcRenderer.invoke("forge:webexplorer-show", bounds)');
    expect(preloadCjsSource).toContain("updateNativeWebExplorerBounds(bounds)");
    expect(preloadCjsSource).toContain('ipcRenderer.invoke("forge:webexplorer-bounds", bounds)');
    expect(preloadCjsSource).toContain("hideNativeWebExplorer()");
    expect(preloadCjsSource).toContain('ipcRenderer.invoke("forge:webexplorer-hide")');
    expect(preloadCjsSource).toContain("onNativeWebExplorerCodeAct(listener)");
    expect(canvasSource).toContain("syncNativeWebExplorerBounds");
    expect(canvasSource).toContain("nativeWebExplorerAccepted");
    expect(canvasSource).toContain("nativeWebExplorerStatus");
    expect(canvasSource).toContain("nativeWebExplorerLastBoundsRef");
    expect(canvasSource).toContain("nativeWebExplorerMotionSyncRef");
    expect(canvasSource).toContain("useLayoutEffect");
    expect(canvasSource).toContain("runMotionSync(420)");
    expect(canvasSource).toContain("leftPanelOpen");
    expect(stylesSource).toContain(".canvasSurfaces--webExplorerOpen");
    expect(stylesSource).toContain("top: 96px");
    expect(stylesSource).toContain("bottom: var(--chat-canvas-bottom)");
    expect(stylesSource).toContain("transition: none");
    expect(stylesSource).toContain("inset: 0 24px");
    expect(canvasSource).toContain("webExplorerNativeSlot--accepted");
    expect(canvasSource).toContain("settleTimers.push");
    expect(canvasSource).toContain("[webexplorer] native view rejected");
  });

  it("restores a persisted chat transcript when a sidebar session is opened", () => {
    expect(mainSource).toContain("function restoreChatSessionToCanvas");
    expect(mainSource).toContain("let chatArchiveLoadPromise");
    expect(mainSource).toContain("await loadChatArchive()");
    expect(mainSource).toContain("materializeOpenedChatSession(command.sessionId, command.section)");
    expect(mainSource).toContain("restoreChatSessionToCanvas(command.sessionId)");
    expect(mainSource).toContain("clearPanelsChatSessionForId(command.sessionId)");
    expect(mainSource).toContain("syncLocalChatSessionsFromArchive()");
    expect(mainSource).toContain("for (const attachment of message.attachments ?? [])");
    expect(mainSource).toContain("(message.attachments ?? []).map(publicArchiveAttachmentPreview)");
    expect(mainSource).toContain("resetPanelsChatSessionView()");
    expect(mainSource).toContain("generateChatSessionId()");
    expect(mainSource).toContain('message.role === "user" || message.role === "assistant"');
    expect(sidebarSource).toContain("await panelsChatBottomStore.refresh()");
  });

  it("unmounts the welcome GPU cover once the selected session has messages", () => {
    expect(appSource).toContain("const sessionHasStarted = useMemo");
    expect(appSource).toContain("panelsChatSnapshot.transcript.some");
    expect(appSource).toContain('message.role === "user" || message.role === "assistant"');
    expect(appSource).toContain("!isFullPageCanvas && !sessionHasStarted");
    expect(appSource).toContain("<ProfileCoverBanner");
  });

  it("lets the user send an attachment-only message and keeps sent previews visible", () => {
    expect(rendererSource).toContain("submitText.trim() || uploadPreviews.length > 0");
    expect(rendererSource).toContain("message.attachments ?? []");
    expect(rendererSource).toContain("function TranscriptAttachmentStack");
    expect(rendererSource).toContain("function isTranscriptVisualAttachment");
    expect(rendererSource).toContain("function TranscriptVisualAttachmentEvents");
    expect(rendererSource).toContain("visualAttachmentEventLabel");
    expect(rendererSource).toContain("const visualAttachments = attachments.filter(isTranscriptVisualAttachment)");
    expect(rendererSource).toContain("<TranscriptVisualAttachmentEvents previews={visualAttachments} />");
    expect(rendererSource).toContain("<TranscriptAttachmentStack previews={visualAttachments} onEditImage={onEditImage} />");
    expect(rendererSource).toContain("stage_attachment_for_edit");
    expect(rendererSource).toContain("composerImageEditPill");
    expect(storeSource).toContain("const attachments = state.snapshot.composer.uploadPreviews");
    expect(storeSource).toContain("attachmentIds: attachments.map((attachment) => attachment.id)");
    expect(storeSource).toContain("if (command.kind === \"send_chat\" && !result.accepted)");
    expect(storeSource).toContain("uploadPreviews: []");
  });

  it("shows an assistant writing placeholder while the LLM response is still in flight", () => {
    expect(storeSource).toContain("const pendingAssistantMessage: TranscriptMessage");
    expect(storeSource).toContain("assistant-pending-");
    expect(storeSource).toContain("const nextTranscript =");
    expect(storeSource).toContain(": [...state.snapshot.transcript, optimisticMessage, pendingAssistantMessage]");
    expect(rendererSource).toContain("function PendingAssistantText");
    expect(rendererSource).toContain("assistantPending");
    expect(rendererSource).toContain("transcriptPill--assistantPending");
  });

  it("keeps the canvas scrolled to the newest message while the assistant is writing", () => {
    expect(rendererSource).toContain("function followTranscriptLatest");
    expect(rendererSource).toContain("function latestTranscriptContainerFor");
    expect(rendererSource).toContain("followTranscriptLatest(messagesRef.current, \"smooth\")");
    expect(rendererSource).toContain("followTranscriptLatest(latestTranscriptContainerFor(textRef.current))");
    expect(rendererSource).toContain("[visibleCharacters]");
    expect(stylesSource).toContain("--transcript-bottom-breathing-room: 48px");
    expect(stylesSource).toContain("padding-bottom: var(--transcript-bottom-breathing-room)");
  });

  it("renders assistant markdown as readable headings, lists, and inline emphasis", () => {
    expect(rendererSource).toContain("function assistantMarkdownBlocks");
    expect(rendererSource).toContain("function assistantInlineNodes");
    expect(rendererSource).toContain("function AssistantMarkdownText");
    expect(rendererSource).toContain("assistantText__heading");
    expect(rendererSource).toContain("assistantText__list");
    expect(rendererSource).toContain("AssistantMarkdownText messageId={message.id}");
    expect(stylesSource).toContain(".assistantText__body");
    expect(stylesSource).toContain(".assistantText__heading");
    expect(stylesSource).toContain(".assistantText__list");
    expect(stylesSource).toContain(".assistantText strong");
  });

  it("projects injected attachments into the current session Files pane", () => {
    expect(appSource).toContain("const sessionFiles = useMemo<ComposerUploadPreview[]>");
    expect(appSource).toContain("for (const file of message.attachments ?? [])");
    expect(appSource).toContain("sessionFiles={sessionFiles}");
    expect(rendererSource).toContain("export function UploadPreview");
    expect(canvasSource).toContain("function CanvasFilesPane");
    expect(rendererSource).toContain("export function TranscriptAttachmentEventIcon");
    expect(canvasSource).toContain("TranscriptAttachmentEventIcon");
    expect(canvasSource).toContain("function AllFilesIcon");
    expect(canvasSource).toContain("fileKindIconKind");
    expect(canvasSource).toContain("canvasFileTile");
    expect(canvasSource).toContain("canvasFileTile__caption");
    expect(canvasSource).toContain("<UploadPreview preview={file} />");
    expect(canvasSource).toContain("stageCanvasImageForEdit(file)");
    expect(canvasSource).toContain("<strong>{file.name}</strong>");
    expect(canvasSource).toContain('filter.id === "all" ? <AllFilesIcon />');
    expect(canvasSource).toContain("className=\"canvasFileTile__captionIcon\"");
    expect(canvasSource).toContain("<TranscriptAttachmentEventIcon kind={file.kind} />");
    expect(canvasSource).not.toContain("<span>{file.kind}</span>");
    expect(canvasSource).toContain("const fileCountLabel = files.length === 1 ? \"1 file\" : `${files.length} files`");
    expect(canvasSource).toContain("sessionName={fileCountLabel}");
    expect(canvasSource).toContain("type FileKindFilter = \"all\" | \"document\" | ComposerUploadPreview[\"kind\"]");
    expect(canvasSource).toContain("const FILE_KIND_FILTERS");
    expect(canvasSource).toContain("{ id: \"image\", label: \"Images et photos\" }");
    expect(canvasSource).toContain("{ id: \"video\", label: \"Videos\" }");
    expect(canvasSource).toContain("{ id: \"model3d\", label: \"Objets 3D\" }");
    expect(canvasSource).toContain("{ id: \"document\", label: \"Documents\", kinds: [\"pdf\", \"spreadsheet\", \"text\"] }");
    expect(canvasSource).toContain("{ id: \"chart\", label: \"Graphiques\" }");
    expect(canvasSource).toContain("{ id: \"file\", label: \"Autres fichiers\" }");
    expect(canvasSource).toContain("const [kindFilter, setKindFilter] = useState<FileKindFilter>(\"all\")");
    expect(canvasSource).toContain("counts.set(\"document\", (counts.get(\"document\") ?? 0) + 1)");
    expect(canvasSource).toContain("const acceptedKinds = activeFilter?.kinds");
    expect(canvasSource).toContain("if (acceptedKinds && !acceptedKinds.includes(file.kind))");
    expect(canvasSource).toContain("aria-label=\"Classer les fichiers par type\"");
    expect(canvasSource).toContain("setKindFilter(filter.id)");
    expect(canvasSource).not.toContain("canvasFilesPane__header");
    expect(stylesSource).toContain("grid-template-rows: auto auto auto minmax(0, 1fr)");
    expect(stylesSource).not.toContain(".canvasFilesPane__header");
    expect(stylesSource).toContain(".canvasFilesPane__filters");
    expect(stylesSource).toContain(".canvasFileTypeButton");
    expect(stylesSource).toContain("grid-template-columns: var(--workspace-action-icon-size) auto");
    expect(stylesSource).toContain("width: var(--workspace-action-icon-size)");
    expect(stylesSource).toContain("stroke-width: 2.15");
    expect(stylesSource).toContain(".canvasFileTile__captionIcon");
    expect(stylesSource).toContain("padding: 3px 5px");
    expect(stylesSource).toContain("backdrop-filter: blur(8px)");
    expect(stylesSource).toContain("column-gap: 0");
    expect(stylesSource).toContain("padding: 0 0 16px");
    expect(stylesSource).toContain("margin: 0");
    expect(stylesSource).toContain("box-shadow: none");
  });

  it("keeps visual media in transcript flow while allowing following text to wrap beside it", () => {
    expect(rendererSource).toContain('className = "chatCanvas"');
    expect(rendererSource).not.toContain("stagedVisualAttachments");
    expect(rendererSource).not.toContain("transcriptMediaStage");
    expect(rendererSource).not.toContain("transcriptItem--withVisuals");
    expect(rendererSource).not.toContain("transcriptUserRow--visuals");
    expect(rendererSource).not.toContain("chatCanvas__messages--withVisuals");
    expect(rendererSource).toContain("transcriptFloatMedia");
    expect(stylesSource).toContain("float: right");
    expect(rendererSource).toContain("<TranscriptAttachmentStack previews={visualAttachments} onEditImage={onEditImage} />");
    expect(rendererSource).toContain("imageEditButton--transcript");
    expect(rendererSource).toContain("image added");
    expect(rendererSource).toContain("video added");
    expect(rendererSource).toContain("3D object added");
  });
});

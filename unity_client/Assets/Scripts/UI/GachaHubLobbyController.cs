using System;
using Heav3ns.Choreography;
using Heav3ns.Input;
using Heav3ns.UI.Navigation;
using Heav3ns.Visuals;
using Heav3ns.UnityBridge;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>Screen-space interaction layer over the shared tabletop. Navigation and progression
    /// are kept in separate models so that presentation can be replaced without losing state.</summary>
    [DisallowMultipleComponent]
    public sealed class GachaHubLobbyController : MonoBehaviour
    {
        private HubNavigationRouter _router;
        private HubProgress _progress;
        private HubCardCatalog _catalog;
        private GachaSummonStage _summon;
        private GachaAltarVisuals _altarVisuals;
        private GrimoireDeckStudio _studio;
        private HeroCharacterShowcase _hero;
        private EventCarouselWidget _carousel;
        private PersistentTopHeaderBar _header;
        private PreBattleDrawerController _preBattle;
        private TitleStageController _title;
        private BattlefieldChoreographer _battle;
        private CardInteractionController _interaction;
        private MulliganController _mulligan;
        private VisualEventQueue _queue;
        private GUIStyle _label, _heading, _button, _small, _titleStyle;
        private Font _displayFont;
        private Texture2D _pixel;
        private Material _foilPreview;
        private Material _portraitMaterial;
        private float _oldTimeScale = 1f;
        private string _message;
        private HubCardDefinition _inspected;
        private Vector2 _deckScroll;
        private Vector2? _slashStart;
        private int _hoveredPackCard = -1;
        private bool _missionsOpen;
        private int _selectedStage;
        private int _activeStoryStage;
        private CardVisual _inspectionSource;
        private CardInspectorOverlay _inspector;
        private static readonly Color Ivory = new Color(.96f, .90f, .78f);
        private static readonly Color Gold = new Color(.85f, .66f, .32f);
        private static readonly Color Panel = new Color(.055f, .045f, .09f, .93f);
        private static readonly Color Border = new Color(.51f, .37f, .19f, .9f);

        public void Initialize(TitleStageController title)
        {
            _title = title;
            _router = gameObject.AddComponent<HubNavigationRouter>();
            _summon = gameObject.AddComponent<GachaSummonStage>();
            _studio = gameObject.AddComponent<GrimoireDeckStudio>();
            _hero = gameObject.AddComponent<HeroCharacterShowcase>();
            _carousel = gameObject.AddComponent<EventCarouselWidget>();
            _header = gameObject.AddComponent<PersistentTopHeaderBar>();
            _progress = HubProgress.Load();
            _progress.RefreshDaily(DateTime.UtcNow.ToString("yyyy-MM-dd"));
            _hero.Initialize(_progress);
            _carousel.Initialize(_router);
            _header.Initialize(_progress, _router);
            TextAsset source = Resources.Load<TextAsset>("Hub/CardCatalog");
            _catalog = source != null ? JsonUtility.FromJson<HubCardCatalog>(source.text) : null;
            if (_catalog == null || _catalog.cards == null)
            {
                Debug.LogError("Hub card catalog is missing or invalid.");
                enabled = false;
                return;
            }
            _summon.Initialize(_progress, _catalog.cards);
            _studio.Initialize(_catalog.cards);
            _progress.EnsureStarterDecks(_catalog.cards);
            _router.ScreenChanged += OnScreenChanged;
            _router.PauseChanged += OnPauseChanged;
            _battle = FindAnyObjectByType<BattlefieldChoreographer>();
            _interaction = FindAnyObjectByType<CardInteractionController>();
            _mulligan = FindAnyObjectByType<MulliganController>();
            _queue = FindAnyObjectByType<VisualEventQueue>();
            if (_queue != null) _queue.OnVisualEventDispatched += OnMatchEvent;
            _preBattle = gameObject.AddComponent<PreBattleDrawerController>();
            _preBattle.Initialize(_progress, _router, _queue, _battle);
            _inspector = FindAnyObjectByType<CardInspectorOverlay>();
            _title.ShowTitleDiorama(true);
            HideLegacyControls();
            gameObject.AddComponent<CinematicIntroController>().Initialize(_title.DioramaRoot, _router);
            gameObject.AddComponent<HubModeMedallion3D>().Initialize(_title.DioramaRoot, _router);
            _altarVisuals = gameObject.AddComponent<GachaAltarVisuals>();
            _altarVisuals.Initialize(_router, _summon);
            gameObject.AddComponent<GrimoireCardGrid>().Initialize(_studio, _router);
        }
        private void HideLegacyControls()
        {
            if (_title.PlayMatchButtonTransform != null) _title.PlayMatchButtonTransform.gameObject.SetActive(false);
            if (_title.AiSpeedButtonTransform != null) _title.AiSpeedButtonTransform.gameObject.SetActive(false);
            if (_title.QuitButtonTransform != null) _title.QuitButtonTransform.gameObject.SetActive(false);
        }
        private void OnScreenChanged(HubScreenState from, HubScreenState to)
        {
            _inspected = null;
            CloseInspection();
            if (to == HubScreenState.GrimoireStudio) _progress.RecordGrimoireVisit();
            if (to == HubScreenState.ActiveBattlefield)
            {
                _title.PressPlayMatchButton(false, true);
            }
            else if (from == HubScreenState.ActiveBattlefield)
            {
                _title.ShowTitleDiorama();
                HideLegacyControls();
            }
        }
        private void OnPauseChanged(bool paused)
        {
            if (paused) { _oldTimeScale = Time.timeScale; Time.timeScale = 0f; }
            else Time.timeScale = _oldTimeScale;
            if (_interaction != null && (_mulligan == null || !_mulligan.IsMulliganActive)) _interaction.InteractionEnabled = !paused;
        }
        private void OnDestroy()
        {
            if (_router != null && _router.IsPauseOpen) Time.timeScale = _oldTimeScale;
            if (_pixel != null) Destroy(_pixel);
            if (_foilPreview != null) Destroy(_foilPreview);
            if (_portraitMaterial != null) Destroy(_portraitMaterial);
            if (_inspectionSource != null) Destroy(_inspectionSource.gameObject);
            if (_queue != null) _queue.OnVisualEventDispatched -= OnMatchEvent;
        }
        private void Update()
        {
            if (_missionsOpen && UnityEngine.Input.GetKeyDown(KeyCode.Escape)) _missionsOpen = false;
        }
        private void OnGUI()
        {
            if (_router == null || _progress == null) return;
            _progress.RefreshDaily(DateTime.UtcNow.ToString("yyyy-MM-dd"));
            EnsureStyles();
            float scale = HubViewport.Scale;
            var oldMatrix = GUI.matrix;
            GUI.matrix = Matrix4x4.TRS(new Vector3(HubViewport.Left, HubViewport.Top, 0), Quaternion.identity, new Vector3(scale, scale, 1));
            GUI.depth = -100;
            if (_router.Current == HubScreenState.SplashScreen) DrawSplash();
            else if (_router.Current == HubScreenState.ActiveBattlefield) { if (_router.IsPauseOpen) DrawPause(); }
            else
            {
                bool physicalCards = _router.Current == HubScreenState.GachaSummonAltar || _router.Current == HubScreenState.GrimoireStudio;
                Box(new Rect(0, 0, 1600, 900), new Color(.025f, .018f, .042f, physicalCards ? .30f : .82f));
                bool previousEnabled = GUI.enabled;
                if (_missionsOpen) GUI.enabled = false;
                DrawHeader();
                switch (_router.Current)
                {
                    case HubScreenState.HubLobby: DrawLobby(); break;
                    case HubScreenState.PreBattleDrawer: DrawPreBattle(); break;
                    case HubScreenState.GachaSummonAltar: DrawSummon(); break;
                    case HubScreenState.GrimoireStudio: DrawStudio(); break;
                }
                GUI.enabled = previousEnabled;
                if (_missionsOpen) DrawMissions();
            }
            if (!string.IsNullOrEmpty(GUI.tooltip))
            {
                Vector2 p = Event.current.mousePosition;
                var tip = new Rect(Mathf.Clamp(p.x + 16, 0, 1240), Mathf.Clamp(p.y + 18, 0, 820), 350, 60);
                Plate(tip); Text(new Rect(tip.x + 10, tip.y + 8, 330, 46), GUI.tooltip, _small);
            }
            GUI.matrix = oldMatrix;
        }
        private void EnsureStyles()
        {
            if (_pixel != null) return;
            _pixel = new Texture2D(1, 1); _pixel.SetPixel(0, 0, Color.white); _pixel.Apply();
            _label = new GUIStyle(GUI.skin.label) { fontSize = 22, wordWrap = true, normal = { textColor = Ivory } };
            _heading = new GUIStyle(_label) { fontSize = 38, fontStyle = FontStyle.Bold };
            _displayFont = Font.CreateDynamicFontFromOSFont("Georgia", 96);
            _titleStyle = new GUIStyle(_label) { font = _displayFont, fontSize = 100, fontStyle = FontStyle.Bold, alignment = TextAnchor.MiddleCenter };
            _titleStyle.normal.textColor = Ivory;
            _small = new GUIStyle(_label) { fontSize = 16 };
            _button = new GUIStyle(GUI.skin.label) { fontSize = 22, fontStyle = FontStyle.Bold, alignment = TextAnchor.MiddleLeft, padding = new RectOffset(20, 12, 6, 6), wordWrap = true };
            _button.normal.textColor = Ivory; _button.hover.textColor = Color.white; _button.active.textColor = Gold;
        }
        private void Box(Rect rect, Color color)
        {
            Color old = GUI.color; GUI.color = color; GUI.DrawTexture(rect, _pixel); GUI.color = old;
        }
        private void Plate(Rect r)
        {
            Box(r, Border); Box(new Rect(r.x + 2, r.y + 2, r.width - 4, r.height - 4), Panel);
        }
        private bool Button(Rect rect, string text, string tooltip = null)
        {
            bool previous = GUI.enabled; GUI.enabled = previous && !_router.IsTransitioning;
            bool hovered = GUI.enabled && rect.Contains(Event.current.mousePosition);
            Box(rect, hovered ? Gold : Border);
            Box(new Rect(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2), hovered ? new Color(.22f, .13f, .12f, .98f) : new Color(.09f, .065f, .105f, .96f));
            bool clicked = GUI.Button(rect, new GUIContent(text, tooltip), _button); GUI.enabled = previous; return clicked;
        }
        private bool TerminalButton(Rect rect, string text)
        {
            if (rect.Contains(Event.current.mousePosition)) rect.x -= 18;
            Matrix4x4 previous = GUI.matrix;
            Matrix4x4 skew = Matrix4x4.identity;
            skew.m01 = Mathf.Tan(12f * Mathf.Deg2Rad);
            skew.m03 = -skew.m01 * rect.center.y;
            GUI.matrix = previous * skew;
            bool clicked = Button(rect, text);
            GUI.matrix = previous;
            return clicked;
        }
        private void Text(Rect r, string s, GUIStyle style = null) => GUI.Label(r, s, style ?? _label);
        private void DrawSplash()
        {
            Box(new Rect(0, 0, 1600, 900), new Color(.035f, .025f, .055f, .76f));
            _titleStyle.normal.textColor = new Color(.40f, .22f, .08f, 1f);
            Text(new Rect(356, 262, 900, 160), "HEAV3NS", _titleStyle);
            _titleStyle.normal.textColor = new Color(.98f, .88f, .62f, 1f);
            Text(new Rect(350, 255, 900, 160), "HEAV3NS", _titleStyle);
            Text(new Rect(485, 425, 850, 50), "WONDERLAND CHRONICLES  //  ASTRAL SEQUENCE", _label);
            bool ready = FindAnyObjectByType<TabletopAudioAndProps>() != null && _catalog.cards.Length > 0;
            Text(new Rect(1190, 45, 350, 35), ready ? "AUDIO  /  CACHE VERIFIED" : "CHECKING CLIENT RESOURCES", _small);
            if (Button(new Rect(430, 685, 740, 70), "CLICK OR PRESS ANY KEY TO AWAKEN")) _router.NavigateTo(HubScreenState.HubLobby);
            Text(new Rect(50, 845, 800, 30), "VER 1.4.0_ASTRAL  //  RUST CORE", _small);
            if (Event.current.type == EventType.KeyDown && Event.current.keyCode != KeyCode.Escape && !_router.IsTransitioning)
            { _router.NavigateTo(HubScreenState.HubLobby); Event.current.Use(); }
            if (Event.current.type == EventType.MouseDown && Event.current.button == 0 && !_router.IsTransitioning)
            { _router.NavigateTo(HubScreenState.HubLobby); Event.current.Use(); }
        }
        private void DrawHeader()
        {
            Plate(new Rect(28, 20, 730, 76)); Plate(new Rect(795, 20, 777, 76));
            if (_router.Current == HubScreenState.HubLobby)
            {
                Text(new Rect(50, 36, 680, 45), (_progress.Data.whiteRabbit ? "WHITE RABBIT" : "ALICE_CHRONICLE") + "  //  LV. 24    ASTRAL CROWN IV");
                Box(new Rect(52, 77, 280, 5), new Color(.2f, .16f, .15f)); Box(new Rect(52, 77, 175, 5), Gold);
            }
            else
            {
                if (Button(new Rect(42, 30, 145, 54), "‹ BACK")) _router.NavigateBack();
                if (Button(new Rect(193, 30, 145, 54), "HOME")) _router.NavigateHome();
                string module = _router.Current == HubScreenState.PreBattleDrawer ? "PRE-BATTLE" : _router.Current == HubScreenState.GachaSummonAltar ? "ASTRAL SUMMON" : "GRIMOIRE STUDIO";
                Text(new Rect(360, 44, 390, 40), "//  " + module, _small);
            }
            GUI.Label(new Rect(820, 43, 210, 40), new GUIContent($"TEA  {_header.Tea} / 120  {_header.RegenLabel}", "Wonderland Tea regenerates one point every five minutes."), _small);
            GUI.Label(new Rect(1040, 43, 210, 40), new GUIContent($"QUARTZ  {_progress.Data.quartz:N0}", "Astral Quartz can be used for a summon when you have no keys."), _small);
            if (Button(new Rect(1260, 30, 285, 54), $"KEYS  {_progress.Data.keys}   +", "Chronicle Keys open packs. Select + to visit the altar."))
            {
                _router.OpenSummonFromHeader();
            }
        }
        private void DrawLobby()
        {
            Plate(new Rect(50, 150, 570, 550));
            Text(new Rect(88, 184, 460, 70), _progress.Data.whiteRabbit ? "WHITE RABBIT" : "ALICE", _heading);
            Texture2D portrait = _hero.Portrait;
            if (portrait != null)
            {
                float breath = Mathf.Sin(Time.unscaledTime * 1.7f) * 3f + _hero.Bounce * 8f;
                float gaze = Mathf.Clamp((Event.current.mousePosition.x - 400f) / 120f, -4f, 4f);
                Rect imageRect = new Rect(105 + gaze, 190 - breath, 440, 390 + breath);
                Shader shader = Shader.Find("Heav3ns/AnimeStandeeBreathing");
                if (shader != null && Event.current.type == EventType.Repaint)
                {
                    if (_portraitMaterial == null) _portraitMaterial = new Material(shader);
                    _portraitMaterial.SetVector("_Gaze", new Vector4(gaze, 0, 0, 0));
                    DrawMaterialTexture(imageRect, portrait, _portraitMaterial);
                }
                else if (shader == null) GUI.DrawTexture(imageRect, portrait, ScaleMode.ScaleToFit);
            }
            if (!_router.IsTransitioning && GUI.Button(new Rect(95, 250, 450, 315), GUIContent.none, GUIStyle.none)) _hero.React();
            Plate(new Rect(75, 570, 500, 95));
            Text(new Rect(88, 585, 470, 65), "“" + _hero.Dialogue + "”");
            Plate(new Rect(850, 155, 680, 195));
            Text(new Rect(880, 175, 600, 48), "WONDERLAND DUEL", _heading);
            Text(new Rect(885, 228, 520, 35), "Alice  vs.  White Rabbit", _small);
            if (Button(new Rect(885, 275, 600, 55), "BATTLE  ›")) { _selectedStage = 0; _router.NavigateTo(HubScreenState.PreBattleDrawer); }
            if (TerminalButton(new Rect(875, 385, 610, 72), "CHRONICLE  //  STORY ENCOUNTERS")) { _selectedStage = 1; _router.NavigateTo(HubScreenState.PreBattleDrawer); }
            if (TerminalButton(new Rect(875, 475, 610, 72), $"GRIMOIRE  //  COLLECTION   {_catalog.cards.Length}")) _router.NavigateTo(HubScreenState.GrimoireStudio);
            if (TerminalButton(new Rect(875, 565, 610, 72), "ASTRAL SUMMON  //  PACK ALTAR")) _router.NavigateTo(HubScreenState.GachaSummonAltar);
            if (Button(new Rect(50, 735, 570, 72), _carousel.Title)) { if (_carousel.Index == 1) _selectedStage = 1; _carousel.Activate(); }
            Box(new Rect(555, 791, 10, 4), _carousel.Index == 0 ? Gold : Border);
            Box(new Rect(575, 791, 10, 4), _carousel.Index == 1 ? Gold : Border);
            if (Button(new Rect(720, 785, 270, 60), "SWITCH ATTENDANT")) _hero.SwitchLeader();
            if (Button(new Rect(1010, 785, 280, 60), _title.FastAiSpeed ? "AI SPEED  //  FAST" : "AI SPEED  //  NORMAL")) _title.ToggleAiSpeedButton();
            if (Button(new Rect(1310, 785, 220, 60), "EXIT TO TITLE")) _router.NavigateTo(HubScreenState.SplashScreen);
            if (Button(new Rect(720, 705, 365, 60), "DAILY CHRONICLE")) _missionsOpen = true;
        }
        private void DrawMissions()
        {
            Box(new Rect(0, 0, 1600, 900), new Color(0, 0, 0, .7f)); Plate(new Rect(420, 205, 760, 490));
            Text(new Rect(470, 245, 650, 55), "DAILY CHRONICLE", _heading);
            Text(new Rect(480, 335, 620, 45), (_progress.Data.dailyVisitedGrimoire ? "[✓]" : "[  ]") + "  Visit the Grimoire");
            Text(new Rect(480, 405, 620, 45), (_progress.Data.dailySummoned ? "[✓]" : "[  ]") + "  Summon a pack");
            Text(new Rect(480, 480, 620, 45), "REWARD  //  160 QUARTZ + 1 KEY", _small);
            if (Button(new Rect(470, 565, 300, 60), _progress.Data.dailyClaimed ? "CLAIMED" : "CLAIM REWARD")) _progress.ClaimDaily();
            if (Button(new Rect(790, 565, 300, 60), "CLOSE")) _missionsOpen = false;
        }
        private void DrawPreBattle()
        {
            Plate(new Rect(230, 150, 1140, 650));
            Text(new Rect(275, 180, 950, 60), "PREPARE FOR DUEL", _heading);
            Text(new Rect(280, 265, 950, 100), _progress.Data.whiteRabbit ? "WHITE RABBIT  //  CHRONO AGGRO\nOpponent: Alice" : "ALICE  //  SKY SEQUENCE CONTROL\nOpponent: White Rabbit", _label);
            Text(new Rect(280, 385, 900, 100), "SIGNATURE CARD  //  CHECKMATE THE SKY\nStarting shields: 6", _label);
            Text(new Rect(280, 510, 900, 60), $"{_progress.ActiveDeck.Count} / 30 cards   //   " + (_title.FastAiSpeed ? "AI pacing: fast 1.8x" : "AI pacing: normal 1.0x"), _label);
            if (_selectedStage > 0)
            {
                if (Button(new Rect(280, 580, 280, 52), "1-1  LOOKING GLASS")) _selectedStage = 1;
                if (Button(new Rect(570, 580, 280, 52), "1-2  COURTYARD")) _selectedStage = 2;
                if (Button(new Rect(860, 580, 300, 52), "1-3  CHECKMATE [BOSS]")) _selectedStage = 3;
                Text(new Rect(280, 650, 450, 45), $"STAGE 1-{_selectedStage}  //  160 QUARTZ" + (_selectedStage == 3 ? " + KEY" : ""), _small);
            }
            if (Button(new Rect(780, 665, 520, 80), "COMMENCE WONDERLAND DUEL"))
            {
                CommenceDuel();
            }
            if (!string.IsNullOrEmpty(_message)) Text(new Rect(280, 615, 900, 40), _message, _small);
        }
        private void DrawPause()
        {
            Box(new Rect(0, 0, 1600, 900), new Color(0, 0, 0, .65f)); Plate(new Rect(440, 225, 720, 440));
            Text(new Rect(490, 260, 600, 60), "TACTICAL PAUSE", _heading);
            if (Button(new Rect(500, 350, 600, 65), "RESUME")) _router.SetPause(false);
            if (Button(new Rect(500, 445, 600, 65), "CONCEDE & RETURN TO LOBBY"))
            {
                ReturnToLobby();
            }
        }
        public bool CommenceDuel()
        {
            if (_preBattle == null || !_preBattle.Commence(_selectedStage)) { _message = _preBattle != null ? _preBattle.Error : "Battlefield is unavailable."; return false; }
            _activeStoryStage = _selectedStage;
            return true;
        }
        public bool ReturnToLobby()
        {
            if (_router == null || _router.IsTransitioning || _router.Current != HubScreenState.ActiveBattlefield) return false;
            _activeStoryStage = 0;
            if (_mulligan != null) _mulligan.CancelOpeningMulligan();
            if (_battle != null) _battle.HandleMatchResetRequested();
            return _router.ReturnFromBattle();
        }
        private void OnMatchEvent(VisualEventRecord evt)
        {
            if (_activeStoryStage > 0 && (evt.eventType == "MatchWon" || evt.eventType == "GameOver") && evt.seat == 0)
                _progress.ClaimStoryVictory(_activeStoryStage);
        }
        private void DrawSummon()
        {
            Plate(new Rect(45, 125, 1510, 150));
            Text(new Rect(85, 150, 1200, 55), "CELESTIAL WONDERLAND", _heading);
            Text(new Rect(88, 215, 1100, 35), $"RATE UP: CHECKMATE THE SKY     //     SSR GUARANTEED WITHIN {10 - _progress.Data.pullsSinceSsr} PULLS", _small);
            if (!_summon.HasPack)
            {
                Plate(new Rect(610, 300, 380, 310));
                Text(new Rect(670, 380, 270, 100), "ASTRAL\nBOOSTER", _heading);
                if (Button(new Rect(520, 685, 560, 80), "SUMMON 1x  //  1 KEY OR 160 QUARTZ"))
                    _message = _summon.PurchasePack() ? "" : "Not enough keys or quartz.";
            }
            else if (!_summon.IsUnsealed)
            {
                Text(new Rect(630, 290, 360, 40), "SLASH THE PACK TO OPEN", _small);
                HandlePackSlash(new Rect(610, 300, 380, 310));
                if (Button(new Rect(610, 680, 380, 75), "UNSEAL PACK")) _summon.Unseal();
            }
            else
            {
                if (Event.current.type == EventType.Repaint && !new Rect(90, 325, 1440, 382).Contains(Event.current.mousePosition))
                { _hoveredPackCard = -1; if (_altarVisuals != null) _altarVisuals.SetHoveredCard(-1); }
                if (!_summon.IsComplete && Button(new Rect(1260, 180, 230, 58), "REVEAL ALL")) _summon.RevealAll();
                if (_summon.IsComplete) Text(new Rect(90, 270, 800, 40), "SUMMON RECAP  //  CARDS ADDED TO COLLECTION", _small);
                for (int i = 0; i < _summon.Cards.Count; i++)
                {
                    float arc = Mathf.Abs(i - 2) * Mathf.Abs(i - 2) * 8f;
                    Rect cardRect = new Rect(90 + i * 295, 325 + arc, 260, 350);
                    var result = _summon.Cards[i];
                    var mouse = Event.current.mousePosition;
                    bool hovered = cardRect.Contains(mouse);
                    if (Event.current.type == EventType.Repaint && hovered && !_summon.IsRevealed(i) && _hoveredPackCard != i)
                    {
                        _hoveredPackCard = i;
                        if (_altarVisuals != null) _altarVisuals.SetHoveredCard(i);
                    }
                    if (!_summon.IsRevealed(i) && hovered)
                    {
                        Color glow = result.Rarity == SummonRarity.Ssr ? Color.HSVToRGB(Mathf.Repeat(Time.unscaledTime * .25f, 1f), .75f, 1f) : result.Rarity == SummonRarity.SuperRare ? Gold : Color.cyan;
                        Box(new Rect(cardRect.x - 5, cardRect.y - 5, cardRect.width + 10, 5), glow);
                        Box(new Rect(cardRect.x - 5, cardRect.yMax, cardRect.width + 10, 5), glow);
                        Box(new Rect(cardRect.x - 5, cardRect.y, 5, cardRect.height), glow);
                        Box(new Rect(cardRect.xMax, cardRect.y, 5, cardRect.height), glow);
                    }
                    if (_altarVisuals == null) DrawCard(cardRect, _summon.IsRevealed(i) ? result.Card : null);
                    if (!_summon.IsRevealed(i) && !_router.IsTransitioning && GUI.Button(cardRect, GUIContent.none, GUIStyle.none)) _summon.Reveal(i);
                }
                if (_summon.IsComplete && !_summon.IsRevealing)
                {
                    if (Button(new Rect(370, 720, 400, 64), "CONTINUE")) _summon.Continue();
                    if (Button(new Rect(790, 720, 440, 64), "SUMMON AGAIN")) _message = _summon.SummonAgain() ? "" : "Not enough keys or quartz.";
                }
            }
            if (!string.IsNullOrEmpty(_message)) Text(new Rect(100, 765, 1000, 40), _message, _small);
        }
        private void HandlePackSlash(Rect pack)
        {
            Event e = Event.current;
            if (_router.IsTransitioning) return;
            if (e.type == EventType.MouseDown && e.button == 0 && pack.Contains(e.mousePosition)) { _slashStart = e.mousePosition; e.Use(); }
            else if (e.type == EventType.MouseUp && e.button == 0 && _slashStart.HasValue)
            {
                if (Vector2.Distance(_slashStart.Value, e.mousePosition) > 100f) _summon.Unseal();
                _slashStart = null; e.Use();
            }
            else if (e.type == EventType.Repaint && _slashStart.HasValue)
            {
                Vector2 end = e.mousePosition, delta = end - _slashStart.Value;
                var old = GUI.matrix;
                GUIUtility.RotateAroundPivot(Mathf.Atan2(delta.y, delta.x) * Mathf.Rad2Deg, _slashStart.Value);
                Box(new Rect(_slashStart.Value.x, _slashStart.Value.y - 2, delta.magnitude, 4), Gold);
                GUI.matrix = old;
            }
        }
        private void DrawCard(Rect rect, HubCardDefinition card)
        {
            string key = card == null ? "card-back" : card.id;
            Texture2D texture = Resources.Load<Texture2D>("Hub/Cards/" + key);
            if (texture != null && card != null && _router.Current == HubScreenState.GrimoireStudio && _studio.FoilPreview && Event.current.type == EventType.Repaint)
            {
                Shader shader = Shader.Find("Heav3ns/CardSurfaceFoil");
                if (shader != null)
                {
                    if (_foilPreview == null) { _foilPreview = new Material(shader); _foilPreview.SetFloat("_FoilIntensity", 1.2f); }
                    DrawMaterialTexture(rect, texture, _foilPreview);
                }
                else GUI.DrawTexture(rect, texture, ScaleMode.ScaleToFit);
            }
            else if (texture != null) GUI.DrawTexture(rect, texture, ScaleMode.ScaleToFit);
            else { Plate(rect); Text(new Rect(rect.x + 15, rect.y + 20, rect.width - 30, rect.height - 40), card == null ? "✦" : card.title, _small); }
        }
        private static void DrawMaterialTexture(Rect rect, Texture2D texture, Material material)
        {
            float ratio = (float)texture.width / texture.height;
            float width = Mathf.Min(rect.width, rect.height * ratio);
            float height = width / ratio;
            Graphics.DrawTexture(new Rect(rect.x + (rect.width - width) / 2f, rect.y + (rect.height - height) / 2f, width, height), texture, material);
        }
        private void DrawStudio()
        {
            Plate(new Rect(35, 115, 1530, 210));
            Text(new Rect(70, 135, 700, 55), "THE GRIMOIRE", _heading);
            if (Button(new Rect(70, 205, 215, 50), "ALL CARDS")) _studio.SetFaction(GrimoireFaction.All);
            if (Button(new Rect(290, 205, 240, 50), "ALICE")) _studio.SetFaction(GrimoireFaction.Alice);
            if (Button(new Rect(535, 205, 250, 50), "WHITE RABBIT")) _studio.SetFaction(GrimoireFaction.WhiteRabbit);
            for (int i = 0; i <= 6; i++)
                if (Button(new Rect(70 + i * 90, 270, 85, 48), i == 0 ? "ALL" : i == 6 ? "6+" : i.ToString())) _studio.SetCost(i);
            if (Button(new Rect(720, 270, 300, 48), _studio.FoilPreview ? "FOIL  //  ON" : "FOIL  //  OFF")) _studio.SetFoil(!_studio.FoilPreview);
            var cards = _studio.FilteredCards;
            for (int i = 0; i < 8; i++)
            {
                int index = _studio.Page * 8 + i;
                if (index >= cards.Count) break;
                var rect = new Rect(70 + (i % 4) * 218, 335 + (i / 4) * 195, 190, 185);
                if (!_router.IsTransitioning && GUI.Button(rect, GUIContent.none, GUIStyle.none)) Inspect(cards[index]);
            }
            if (cards.Count == 0) Text(new Rect(80, 380, 800, 60), "No cards match these filters.");
            if (Button(new Rect(70, 752, 170, 50), "‹ PREV")) _studio.SetPage(_studio.Page - 1);
            Text(new Rect(270, 768, 220, 30), $"PAGE {_studio.Page + 1} / {_studio.PageCount}", _small);
            if (Button(new Rect(510, 752, 170, 50), "NEXT ›")) _studio.SetPage(_studio.Page + 1);
            Plate(new Rect(1035, 205, 470, 590));
            Text(new Rect(1070, 235, 390, 45), _inspected != null ? _inspected.title : "MANA CURVE", _label);
            if (_inspected != null)
            {
                Plate(new Rect(310, 330, 430, 415));
                DrawCard(new Rect(325, 340, 400, 390), _inspected);
                Text(new Rect(1070, 310, 380, 230), $"{_inspected.deck}  •  COST {_inspected.cost}\n\n{_inspected.type}\n\n{_inspected.text}", _small);
                if (Button(new Rect(1070, 565, 300, 50), "ADD TO ACTIVE DECK")) _message = _progress.AddToDeck(_inspected) ? "Added to deck." : "Deck full, wrong faction, or copy limit reached.";
                if (Button(new Rect(1070, 625, 300, 50), "REMOVE FROM DECK")) _message = _progress.RemoveFromDeck(_inspected.id) ? "Removed from deck." : "That card is not in your deck.";
                if (!string.IsNullOrEmpty(_message)) Text(new Rect(1070, 680, 380, 35), _message, _small);
                if (Button(new Rect(1070, 710, 300, 55), "CLOSE DETAILS")) CloseInspection();
            }
            else
            {
                var curve = _studio.ManaCurveForDeck(_progress.ActiveDeck);
                int max = 1; foreach (int count in curve) max = Mathf.Max(max, count);
                for (int i = 0; i < curve.Length; i++)
                {
                    float height = 120f * curve[i] / max;
                    Box(new Rect(1075 + i * 55, 435 - height, 35, height), Gold);
                    Text(new Rect(1078 + i * 55, 442, 45, 30), i == 6 ? "7+" : (i + 1).ToString(), _small);
                    Text(new Rect(1078 + i * 55, 405 - height, 45, 30), curve[i].ToString(), _small);
                }
                Text(new Rect(1070, 485, 390, 35), (_progress.Data.whiteRabbit ? "RABBIT" : "ALICE") + $" DECK  //  {_progress.ActiveDeck.Count} / 30", _small);
                int deckEntries = 0; foreach (var card in _catalog.cards) if (_progress.ActiveDeck.Contains(card.id)) deckEntries++;
                _deckScroll = GUI.BeginScrollView(new Rect(1065, 525, 410, 230), _deckScroll, new Rect(0, 0, 375, deckEntries * 30));
                int row = 0;
                foreach (var card in _catalog.cards)
                {
                    int copies = _progress.ActiveDeck.FindAll(id => id == card.id).Count;
                    if (copies == 0) continue;
                    Text(new Rect(5, row++ * 30, 355, 28), $"{card.cost}    {card.title}   ×{copies}", _small);
                }
                GUI.EndScrollView();
            }
        }
        private void Inspect(HubCardDefinition card)
        {
            _inspected = card;
            _message = null;
            if (_inspector == null) return;
            if (_inspectionSource == null)
            {
                _inspectionSource = CardVisual.ConstructProceduralCardObject("Grimoire inspection source");
                _inspectionSource.transform.position = new Vector3(0, -1000, 0);
                _inspectionSource.Spring.SetTarget(_inspectionSource.transform.position, Quaternion.identity, Vector3.one, true);
                _inspectionSource.Collider.enabled = false;
            }
            _inspectionSource.ConfigureCard(0, card.id, card.deck == "Alice" ? 0 : 1, card.title, card.type, card.cost, card.text, 0, 1, false, "collection", false);
            _inspector.InspectCard(_inspectionSource);
        }
        private void CloseInspection()
        {
            _inspected = null;
            if (_inspector != null) _inspector.HideInspection();
        }
    }
}

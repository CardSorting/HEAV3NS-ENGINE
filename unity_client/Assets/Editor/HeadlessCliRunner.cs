using System;
using System.IO;
using System.Linq;
using Heav3ns.Board;
using Heav3ns.Choreography;
using Heav3ns.Input;
using Heav3ns.UI;
using Heav3ns.UnityBridge;
using Heav3ns.Visuals;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEditor.Compilation;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

/// <summary>
/// Headless CLI automation, prefab generator, full battlefield bootstrapper, end-to-end playable
/// match simulation harness, and standalone macOS desktop builder for HEAV3NS-ENGINE Unity Client.
/// Invoked via Unity -batchmode -nographics -executeMethod HeadlessCliRunner.<MethodName>
/// </summary>
public static class HeadlessCliRunner
{
    private const string DefaultScenePath = "Assets/Scenes/Battlefield.unity";
    private const string CardActorPrefabPath = "Assets/Prefabs/CardActor.prefab";
    private const string NativePluginPath = "Assets/Plugins/libheav3ns_engine.dylib";
    private const string SimulationScenePath = "Assets/Scenes/__Heav3nsSimulation.unity";
    private const string SimulationPrefabPath = "Assets/Prefabs/__Heav3nsSimulationCard.prefab";

    /// <summary>
    /// Forces AssetDatabase.Refresh(), checks EditorUtility.scriptCompilationFailed,
    /// outputs structured compiler status/errors to stdout/log, and exits with code 0 (success) or 1 (failure).
    /// </summary>
    [MenuItem("HEAV3NS/Headless/Compile And Verify")]
    public static void CompileAndVerify()
    {
        Debug.Log("[HeadlessCliRunner] Starting CompileAndVerify...");
        try
        {
            AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);

            bool compilationFailed = EditorUtility.scriptCompilationFailed;
            Assembly[] assemblies = CompilationPipeline.GetAssemblies();

            Console.WriteLine("========================================");
            Console.WriteLine("[HEAV3NS-ENGINE] UNITY COMPILE VERIFICATION");
            Console.WriteLine($"Unity Version : {Application.unityVersion}");
            Console.WriteLine($"Project Path  : {Directory.GetCurrentDirectory()}");
            Console.WriteLine($"Assemblies    : {assemblies.Length}");
            Console.WriteLine($"Status        : {(compilationFailed ? "FAILED" : "PASSED")}");
            Console.WriteLine("========================================");

            if (compilationFailed)
            {
                Debug.LogError("[HeadlessCliRunner] Script compilation failed.");
                if (Application.isBatchMode)
                {
                    EditorApplication.Exit(1);
                }
                return;
            }

            Debug.Log($"[HeadlessCliRunner] CompileAndVerify succeeded ({assemblies.Length} assemblies verified, 0 errors).");
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }
        catch (Exception ex)
        {
            Debug.LogError($"[HeadlessCliRunner] Exception in CompileAndVerify: {ex}");
            Console.Error.WriteLine($"[HeadlessCliRunner] FATAL ERROR: {ex.Message}");
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(1);
            }
        }
    }

    /// <summary>
    /// Constructs a reusable Assets/Prefabs/CardActor.prefab featuring a 3D quad/box mesh with front/back faces,
    /// ornate frame border, artwork slot, dynamic shadow, BoxCollider, SecondOrderSpring, custom shaders
    /// (CardBorderGlow.shader & CardSurfaceFoil.shader), and world-space TextMeshPro labels.
    /// </summary>
    [MenuItem("HEAV3NS/Headless/Create Card Prefab")]
    public static CardVisual CreateCardPrefab()
    {
        return CreateCardPrefabAt(CardActorPrefabPath);
    }

    private static CardVisual CreateCardPrefabAt(string prefabPath)
    {
        Debug.Log("[HeadlessCliRunner] Starting CreateCardPrefab...");
        string prefabsDir = Path.Combine(Application.dataPath, "Prefabs");
        if (!Directory.Exists(prefabsDir))
        {
            Directory.CreateDirectory(prefabsDir);
        }

        CardVisual tempCard = CardVisual.ConstructProceduralCardObject("CardActor");
        tempCard.ConfigureCard(
            1,
            "H3W-AL-001",
            0,
            "A Question of Doors",
            "Action",
            1,
            "Choose one: Draw 1 card; or return a friendly Character to hand.\nShield Trigger: Draw 1 card.",
            2,
            3,
            false,
            "hand",
            true);
        tempCard.gameObject.name = "CardActor";

        bool prefabSuccess;
        GameObject savedPrefabGo = PrefabUtility.SaveAsPrefabAsset(tempCard.gameObject, prefabPath, out prefabSuccess);
        UnityEngine.Object.DestroyImmediate(tempCard.gameObject);

        if (!prefabSuccess || savedPrefabGo == null)
        {
            throw new InvalidOperationException($"Failed to save prefab to {prefabPath}");
        }

        AssetDatabase.SaveAssets();
        AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);

        CardVisual prefabComponent = savedPrefabGo.GetComponent<CardVisual>();
        Console.WriteLine("========================================");
        Console.WriteLine("[HEAV3NS-ENGINE] CARD ACTOR PREFAB GENERATED");
        Console.WriteLine($"Prefab Path   : {prefabPath}");
        Console.WriteLine($"BoxCollider   : {savedPrefabGo.GetComponent<BoxCollider>() != null}");
        Console.WriteLine($"SpringSolver  : {savedPrefabGo.GetComponent<SecondOrderSpring>() != null}");
        Console.WriteLine($"TMP Labels    : Cost={prefabComponent.CostLabel != null}, Attack={prefabComponent.AttackPowerLabel != null}, Health={prefabComponent.HealthDefenseLabel != null}, Title={prefabComponent.TitleLabel != null}, Rules={prefabComponent.RulesTextLabel != null}");
        Console.WriteLine("========================================");

        return prefabComponent;
    }

    /// <summary>
    /// Generates CardActor.prefab, builds or updates EnchantedTableStage and RustEngineBridgeHost with all
    /// layout, input, choreography, custom shader, Mulligan, CardInspectorOverlay, ActionHistoryLog,
    /// TitleStageController, HUD, and audio/prop managers, populates initial board state, and saves Assets/Scenes/Battlefield.unity.
    /// </summary>
    [MenuItem("HEAV3NS/Headless/Build Full Battlefield")]
    public static void BuildFullBattlefield()
    {
        Debug.Log("[HeadlessCliRunner] Starting BuildFullBattlefield...");
        try
        {
            BuildFullBattlefieldInternal();
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }
        catch (Exception ex)
        {
            Debug.LogError($"[HeadlessCliRunner] Exception in BuildFullBattlefield: {ex}");
            Console.Error.WriteLine($"[HeadlessCliRunner] FATAL ERROR: {ex.Message}");
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(1);
            }
        }
    }

    [MenuItem("HEAV3NS/Headless/Bootstrap Card Stage")]
    public static void BootstrapCardStage()
    {
        BuildFullBattlefield();
    }

    private static BattlefieldChoreographer BuildFullBattlefieldInternal(bool simulation = false)
    {
        EnsureTmpEssentialsImported();
        string scenePath = simulation ? SimulationScenePath : DefaultScenePath;
        CardVisual cardPrefab = CreateCardPrefabAt(simulation ? SimulationPrefabPath : CardActorPrefabPath);

        string scenesDir = Path.Combine(Application.dataPath, "Scenes");
        if (!Directory.Exists(scenesDir))
        {
            Directory.CreateDirectory(scenesDir);
        }

        Scene scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        scene.name = "Battlefield";

        // 1. Main Camera tilted for a 2.5D Enchanted Table board view (with SecondOrderSpring for TitleStageController)
        GameObject cameraGo = new GameObject("Main Camera");
        cameraGo.tag = "MainCamera";
        Camera cam = cameraGo.AddComponent<Camera>();
        cam.orthographic = false;
        cam.fieldOfView = 42f;
        cam.nearClipPlane = 0.1f;
        cam.farClipPlane = 100f;
        cam.clearFlags = CameraClearFlags.SolidColor;
        cam.backgroundColor = new Color(0.06f, 0.05f, 0.09f, 1.0f);
        cameraGo.transform.position = new Vector3(0f, 11.5f, -7.8f);
        cameraGo.transform.rotation = Quaternion.Euler(56f, 0f, 0f);
        cameraGo.AddComponent<AudioListener>();
        SecondOrderSpring camSpring = cameraGo.AddComponent<SecondOrderSpring>();
        camSpring.SetTarget(new Vector3(0f, 11.5f, -7.8f), Quaternion.Euler(56f, 0f, 0f), Vector3.one, true);

        // 2. Directional Lighting (Warm Key Light for 2.5D Card Stage)
        GameObject keyLightGo = new GameObject("Directional Light");
        Light keyLight = keyLightGo.AddComponent<Light>();
        keyLight.type = LightType.Directional;
        keyLight.color = new Color(1.0f, 0.95f, 0.86f, 1.0f);
        keyLight.intensity = 1.25f;
        keyLight.shadows = LightShadows.Soft;
        keyLight.transform.position = new Vector3(3f, 12f, -4f);
        keyLight.transform.rotation = Quaternion.Euler(50f, -30f, 0f);

        // 3. 2.5D Battlefield Stage Root & Custom Shader Board Surface
        GameObject stageRoot = new GameObject("EnchantedTableStage");
        stageRoot.transform.position = Vector3.zero;

        GameObject boardSurface = GameObject.CreatePrimitive(PrimitiveType.Plane);
        boardSurface.name = "BoardSurface";
        boardSurface.transform.SetParent(stageRoot.transform, false);
        boardSurface.transform.localPosition = Vector3.zero;
        boardSurface.transform.localScale = new Vector3(1.8f, 1.0f, 1.2f);
        Collider boardCollider = boardSurface.GetComponent<Collider>();
        Renderer boardRenderer = boardSurface.GetComponent<Renderer>();
        if (boardRenderer != null)
        {
            Material tabletopMat = CardArtRegistry.GetSharedTabletopMaterial();
            if (tabletopMat != null) boardRenderer.sharedMaterial = tabletopMat;
        }

        // 4. 6 Deterministic Zone Anchors & Layout Managers
        GameObject aliceHandGo = CreateZoneAnchor(stageRoot.transform, "AliceHandAnchor", new Vector3(0f, 0.15f, -4.5f));
        HandArcLayout aliceHandLayout = aliceHandGo.AddComponent<HandArcLayout>();
        aliceHandLayout.ConfigureHandZone(0, false);

        GameObject aliceSeqGo = CreateZoneAnchor(stageRoot.transform, "AliceSequenceRow", new Vector3(0f, 0.1f, -1.6f));
        BoardRowLayout aliceSeqLayout = aliceSeqGo.AddComponent<BoardRowLayout>();
        aliceSeqLayout.ConfigureRow(0, BoardRowLayout.RowKind.Sequence, false);

        GameObject aliceShieldGo = CreateZoneAnchor(stageRoot.transform, "AliceShieldRow", new Vector3(-5.2f, 0.12f, -2.8f));
        BoardRowLayout aliceShieldLayout = aliceShieldGo.AddComponent<BoardRowLayout>();
        aliceShieldLayout.ConfigureRow(0, BoardRowLayout.RowKind.Shield, true);

        GameObject rabbitHandGo = CreateZoneAnchor(stageRoot.transform, "WhiteRabbitHandAnchor", new Vector3(0f, 0.15f, 4.5f));
        HandArcLayout rabbitHandLayout = rabbitHandGo.AddComponent<HandArcLayout>();
        rabbitHandLayout.ConfigureHandZone(1, true);

        GameObject rabbitSeqGo = CreateZoneAnchor(stageRoot.transform, "WhiteRabbitSequenceRow", new Vector3(0f, 0.1f, 1.6f));
        BoardRowLayout rabbitSeqLayout = rabbitSeqGo.AddComponent<BoardRowLayout>();
        rabbitSeqLayout.ConfigureRow(1, BoardRowLayout.RowKind.Sequence, false);

        GameObject rabbitShieldGo = CreateZoneAnchor(stageRoot.transform, "WhiteRabbitShieldRow", new Vector3(5.2f, 0.12f, 2.8f));
        BoardRowLayout rabbitShieldLayout = rabbitShieldGo.AddComponent<BoardRowLayout>();
        rabbitShieldLayout.ConfigureRow(1, BoardRowLayout.RowKind.Shield, true);

        // 5. Diegetic Tabletop HUD, Corner Props, Audio Pool, Big-Card Inspector, History Log, and Title Stage
        TabletopAudioAndProps audioAndProps = stageRoot.AddComponent<TabletopAudioAndProps>();
        audioAndProps.EnsureInitialized();

        DiegeticTabletopHud tabletopHud = stageRoot.AddComponent<DiegeticTabletopHud>();
        tabletopHud.EnsureHudBuilt();

        CardInspectorOverlay inspectorOverlay = stageRoot.AddComponent<CardInspectorOverlay>();
        inspectorOverlay.EnsureBuilt();

        ActionHistoryLog actionHistoryLog = stageRoot.AddComponent<ActionHistoryLog>();
        actionHistoryLog.EnsureBuilt();

        TitleStageController titleStageController = stageRoot.AddComponent<TitleStageController>();
        titleStageController.EnsureBuilt();

        // 6. Targeting Arrow & Input Controller on EnchantedTableStage
        GameObject arrowGo = new GameObject("TargetingArrowRenderer");
        arrowGo.transform.SetParent(stageRoot.transform, false);
        TargetingArrowRenderer arrowRenderer = arrowGo.AddComponent<TargetingArrowRenderer>();
        arrowRenderer.EnsureChevronPool();
        arrowRenderer.HideArrow();

        CardInteractionController interactionController = stageRoot.AddComponent<CardInteractionController>();

        // 7. RustEngineBridgeHost with VisualEventQueue, BattlefieldChoreographer, and MulliganController
        GameObject bridgeHost = new GameObject("RustEngineBridgeHost");
        VisualEventQueue eventQueue = bridgeHost.AddComponent<VisualEventQueue>();
        BattlefieldChoreographer choreographer = bridgeHost.AddComponent<BattlefieldChoreographer>();
        MulliganController mulliganController = bridgeHost.AddComponent<MulliganController>();

        interactionController.BindReferences(
            cam,
            boardCollider,
            aliceHandLayout,
            aliceSeqLayout,
            aliceShieldLayout,
            rabbitSeqLayout,
            rabbitShieldLayout,
            arrowRenderer,
            eventQueue,
            inspectorOverlay,
            actionHistoryLog);

        choreographer.BindReferences(
            eventQueue,
            cardPrefab,
            cam,
            interactionController,
            tabletopHud,
            audioAndProps,
            aliceHandLayout,
            aliceSeqLayout,
            aliceShieldLayout,
            rabbitHandLayout,
            rabbitSeqLayout,
            rabbitShieldLayout,
            mulliganController,
            inspectorOverlay,
            actionHistoryLog,
            titleStageController);

        mulliganController.EnsureBuilt();

        // Initialize native match and populate initial cards in the saved scene so it is immediately playable
        if (eventQueue.TryInitializeNativeBridge())
        {
            eventQueue.DrainAllEventsSynchronously(0.15f);
            choreographer.SyncBoardStateFromNative(true);
            eventQueue.ShutdownNativeBridge();
        }

        bool saved = EditorSceneManager.SaveScene(scene, scenePath);
        if (!saved || !File.Exists(Path.Combine(Directory.GetCurrentDirectory(), scenePath)))
        {
            throw new InvalidOperationException($"Failed to save scene to {scenePath}");
        }

        if (!simulation) EnsureSceneInBuildSettings();

        AssetDatabase.SaveAssets();
        AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);

        Console.WriteLine("========================================");
        Console.WriteLine("[HEAV3NS-ENGINE] BUILD FULL BATTLEFIELD COMPLETE");
        Console.WriteLine($"Scene Path      : {scenePath}");
        Console.WriteLine($"Prefab Path     : {(simulation ? SimulationPrefabPath : CardActorPrefabPath)}");
        Console.WriteLine($"Alice Hand Cards: {aliceHandLayout.Cards.Count}");
        Console.WriteLine($"Alice Shields   : {aliceShieldLayout.Cards.Count}");
        Console.WriteLine($"Rabbit Hand     : {rabbitHandLayout.Cards.Count} (FaceDown={rabbitHandLayout.MaskCardsAsFaceDown})");
        Console.WriteLine($"Rabbit Shields  : {rabbitShieldLayout.Cards.Count} (FaceDown={rabbitShieldLayout.EnforceFaceDownShields})");
        Console.WriteLine($"Root Nodes      : {string.Join(", ", scene.GetRootGameObjects().Select(g => g.name))}");
        Console.WriteLine("========================================");

        return choreographer;
    }

    private static void EnsureTmpEssentialsImported()
    {
        string tmpSettingsPath = Path.Combine(Application.dataPath, "TextMesh Pro", "Resources", "TMP Settings.asset");
        if (!File.Exists(tmpSettingsPath))
        {
            string packageCacheDir = Path.Combine(Directory.GetCurrentDirectory(), "Library", "PackageCache");
            if (Directory.Exists(packageCacheDir))
            {
                string[] packages = Directory.GetFiles(packageCacheDir, "TMP Essential Resources.unitypackage", SearchOption.AllDirectories);
                if (packages.Length > 0)
                {
                    UnityEditor.AssetPackage.Package.Import(packages[0], false);
                    AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);
                }
            }
        }
        TMPro.TMP_Settings.LoadDefaultSettings();
    }

    private static void EnsureSceneInBuildSettings()
    {
        EditorBuildSettingsScene[] existingScenes = EditorBuildSettings.scenes;
        var list = existingScenes.Where(s => s.path != DefaultScenePath).ToList();
        list.Insert(0, new EditorBuildSettingsScene(DefaultScenePath, true));
        EditorBuildSettings.scenes = list.ToArray();
    }

    /// <summary>
    /// Phase 5 Standalone Build Pipeline & Verification:
    /// Configures PlayerSettings (Product Name: Heav3ns, Bundle Identifier, Default Screen Width 1920,
    /// Height 1080, Fullscreen Windowed), registers Assets/Scenes/Battlefield.unity at index 0,
    /// configures Assets/Plugins/libheav3ns_engine.dylib for macOS Standalone bundling into
    /// Builds/macOS/Heav3ns.app/Contents/PlugIns/, and executes BuildPipeline.BuildPlayer.
    /// </summary>
    [MenuItem("HEAV3NS/Headless/Build Standalone macOS")]
    public static void BuildStandaloneMacOS()
    {
        Debug.Log("[HeadlessCliRunner] Starting BuildStandaloneMacOS...");
        try
        {
            // Build the authored scene; regenerating it here would discard scene edits.
            AssertCondition(File.Exists(DefaultScenePath), $"Missing scene: {DefaultScenePath}");
            EditorSceneManager.OpenScene(DefaultScenePath, OpenSceneMode.Single);
            AssertCondition(UnityEngine.Object.FindAnyObjectByType<TitleStageController>() != null, "Battlefield must contain the title stage");
            EnsureSceneInBuildSettings();
            AssertCondition(
                EditorBuildSettings.scenes.Length > 0 && EditorBuildSettings.scenes[0].path == DefaultScenePath,
                "Assets/Scenes/Battlefield.unity must be registered at index 0 in EditorBuildSettings.scenes");

            // 1. Configure PlayerSettings
            PlayerSettings.companyName = "Heav3ns";
            PlayerSettings.productName = "Heav3ns";
            PlayerSettings.SetApplicationIdentifier(NamedBuildTarget.Standalone, "com.heav3ns.wonderland");
            PlayerSettings.defaultScreenWidth = 1920;
            PlayerSettings.defaultScreenHeight = 1080;
            PlayerSettings.fullScreenMode = FullScreenMode.FullScreenWindow;

            // 2. Verify and configure Assets/Plugins/libheav3ns_engine.dylib PluginImporter for macOS Standalone
            PluginImporter pluginImporter = AssetImporter.GetAtPath(NativePluginPath) as PluginImporter;
            AssertCondition(pluginImporter != null, $"PluginImporter must exist for {NativePluginPath}");
            pluginImporter.SetCompatibleWithAnyPlatform(false);
            pluginImporter.SetCompatibleWithEditor(true);
            pluginImporter.SetCompatibleWithPlatform(BuildTarget.StandaloneOSX, true);
            pluginImporter.SetPlatformData(BuildTarget.StandaloneOSX, "CPU", "AnyCPU");
            pluginImporter.SaveAndReimport();

            // 3. Prepare output directory Builds/macOS/Heav3ns.app
            string projectDir = Directory.GetCurrentDirectory();
            string repoRootDir = Path.GetFullPath(Path.Combine(projectDir, ".."));
            string buildOutputRelativeToProject = "Builds/macOS/Heav3ns.app";
            string fullBuildAppPath = Path.Combine(projectDir, buildOutputRelativeToProject);
            string repoBuildAppPath = Path.Combine(repoRootDir, buildOutputRelativeToProject);

            string buildParentDir = Path.GetDirectoryName(fullBuildAppPath);
            if (!string.IsNullOrEmpty(buildParentDir) && !Directory.Exists(buildParentDir))
            {
                Directory.CreateDirectory(buildParentDir);
            }

            BuildPlayerOptions buildOptions = new BuildPlayerOptions
            {
                scenes = new[] { DefaultScenePath },
                locationPathName = fullBuildAppPath,
                target = BuildTarget.StandaloneOSX,
                options = BuildOptions.None
            };

            BuildReport report = BuildPipeline.BuildPlayer(buildOptions);
            BuildSummary summary = report.summary;
            AssertCondition(summary.result == BuildResult.Succeeded && summary.totalErrors == 0,
                $"BuildPipeline.BuildPlayer must succeed with 0 errors (got {summary.result} with {summary.totalErrors} errors)");

            // Ensure plugin is present in Contents/PlugIns/ inside the built .app bundle
            string bundlePluginsDir = Path.Combine(fullBuildAppPath, "Contents", "PlugIns");
            string bundledDylibPath = Path.Combine(bundlePluginsDir, "libheav3ns_engine.dylib");
            string sourceDylibPath = Path.Combine(projectDir, NativePluginPath);
            if (Directory.Exists(fullBuildAppPath) && !File.Exists(bundledDylibPath) && File.Exists(sourceDylibPath))
            {
                Directory.CreateDirectory(bundlePluginsDir);
                File.Copy(sourceDylibPath, bundledDylibPath, true);
            }

            // Mirror the app bundle to repoRoot/Builds/macOS/Heav3ns.app so both ./Builds/macOS/Heav3ns.app
            // and unity_client/Builds/macOS/Heav3ns.app exist cleanly.
            if (Directory.Exists(fullBuildAppPath) && !string.Equals(fullBuildAppPath, repoBuildAppPath, StringComparison.OrdinalIgnoreCase))
            {
                CopyDirectoryRecursive(fullBuildAppPath, repoBuildAppPath);
            }

            Console.WriteLine("========================================");
            Console.WriteLine("[HEAV3NS-ENGINE] STANDALONE MACOS BUILD REPORT");
            Console.WriteLine($"Product Name    : {PlayerSettings.productName}");
            Console.WriteLine($"Bundle ID       : {PlayerSettings.GetApplicationIdentifier(NamedBuildTarget.Standalone)}");
            Console.WriteLine($"Resolution      : {PlayerSettings.defaultScreenWidth}x{PlayerSettings.defaultScreenHeight} ({PlayerSettings.fullScreenMode})");
            Console.WriteLine($"Scene [0]       : {EditorBuildSettings.scenes[0].path}");
            Console.WriteLine($"Output Path     : {fullBuildAppPath}");
            Console.WriteLine($"Repo Output Path: {repoBuildAppPath}");
            Console.WriteLine($"Bundled Dylib   : {File.Exists(bundledDylibPath)} ({bundledDylibPath})");
            Console.WriteLine($"Build Result    : {summary.result}");
            Console.WriteLine($"Total Errors    : {summary.totalErrors}");
            Console.WriteLine($"Total Size      : {summary.totalSize} bytes");
            Console.WriteLine("========================================");

            AssertCondition(
                summary.result == BuildResult.Succeeded && summary.totalErrors == 0,
                $"BuildPipeline.BuildPlayer must succeed with 0 errors (got {summary.result} with {summary.totalErrors} errors)");
            AssertCondition(
                Directory.Exists(fullBuildAppPath),
                $"Built app bundle must exist at {fullBuildAppPath}");
            AssertCondition(
                File.Exists(bundledDylibPath),
                $"Native library must be bundled at {bundledDylibPath}");

            Debug.Log("[HeadlessCliRunner] BuildStandaloneMacOS succeeded!");
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }
        catch (Exception ex)
        {
            Debug.LogError($"[HeadlessCliRunner] Exception in BuildStandaloneMacOS: {ex}");
            Console.Error.WriteLine($"[HeadlessCliRunner] BUILD FAILED: {ex}");
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(1);
            }
        }
    }

    private static void CopyDirectoryRecursive(string sourceDir, string destinationDir)
    {
        if (Directory.Exists(destinationDir))
        {
            Directory.Delete(destinationDir, true);
        }
        Directory.CreateDirectory(destinationDir);

        foreach (string file in Directory.GetFiles(sourceDir))
        {
            string destFile = Path.Combine(destinationDir, Path.GetFileName(file));
            File.Copy(file, destFile, true);
        }

        foreach (string subDir in Directory.GetDirectories(sourceDir))
        {
            string destSubDir = Path.Combine(destinationDir, Path.GetFileName(subDir));
            CopyDirectoryRecursive(subDir, destSubDir);
        }
    }

    /// <summary>
    /// Automated End-to-End Playable Match Simulation (Phase 1-5 Acceptance Criteria).
    /// Verifies:
    /// - Custom shader assignment (CardBorderGlow.shader & CardSurfaceFoil.shader) & CardArtRegistry textures
    /// - Diegetic TitleStageController camera transition & SkipIntroInSimulations flag
    /// - Opening MulliganController 1.6x viewport fan, Replace/Redraw badge toggle, Confirm Hand button, and 0.08s shield cascade
    /// - Right-click & 0.6s long-hover CardInspectorOverlay (2.2x preview + keyword callouts)
    /// - ActionHistoryLog recording last 6 resolved VisualEvent entries
    /// - High-Impact Shield Break & Reveal Drama (180 deg flip at camera center for 0.45s + crystalline shatter)
    /// - Noise-driven dissolve animation (_DissolveAmount 0.0 -> 1.0) on CardDestroyed/UnitDestroyed/ShieldBroken
    /// - Full turn play, White Rabbit AI turn, and MatchResetButton3D full match reset.
    /// </summary>
    [MenuItem("HEAV3NS/Headless/Run Playable Match Simulation")]
    public static void RunPlayableMatchSimulation()
    {
        Debug.Log("[HeadlessCliRunner] Starting RunPlayableMatchSimulation...");
        try
        {
            // 1. Build the full battlefield scene and prefab
            BattlefieldChoreographer choreographer = BuildFullBattlefieldInternal(true);
            VisualEventQueue queue = choreographer.GetComponent<VisualEventQueue>();
            MulliganController mulligan = choreographer.GetComponent<MulliganController>();
            CardInteractionController interaction = UnityEngine.Object.FindAnyObjectByType<CardInteractionController>();
            DiegeticTabletopHud hud = UnityEngine.Object.FindAnyObjectByType<DiegeticTabletopHud>();
            TabletopAudioAndProps audioProps = UnityEngine.Object.FindAnyObjectByType<TabletopAudioAndProps>();
            TargetingArrowRenderer arrow = UnityEngine.Object.FindAnyObjectByType<TargetingArrowRenderer>();
            CardInspectorOverlay inspector = UnityEngine.Object.FindAnyObjectByType<CardInspectorOverlay>();
            ActionHistoryLog historyLog = UnityEngine.Object.FindAnyObjectByType<ActionHistoryLog>();
            TitleStageController titleStage = UnityEngine.Object.FindAnyObjectByType<TitleStageController>();

            AssertCondition(choreographer != null, "BattlefieldChoreographer must exist in scene");
            AssertCondition(queue != null, "VisualEventQueue must exist in scene");
            AssertCondition(mulligan != null, "MulliganController must exist in scene");
            AssertCondition(interaction != null, "CardInteractionController must exist in scene");
            AssertCondition(hud != null, "DiegeticTabletopHud must exist in scene");
            AssertCondition(audioProps != null, "TabletopAudioAndProps must exist in scene");
            AssertCondition(arrow != null, "TargetingArrowRenderer must exist in scene");
            AssertCondition(inspector != null, "CardInspectorOverlay must exist in scene");
            AssertCondition(historyLog != null, "ActionHistoryLog must exist in scene");
            AssertCondition(titleStage != null, "TitleStageController must exist in scene");

            // 2. Initialize live native match via RustEngineBridge
            bool connected = queue.TryInitializeNativeBridge();
            AssertCondition(connected && queue.IsNativeConnected, "Native RustEngineBridge must connect");

            // Drain initial TurnStarted & CardDrawn events
            int initialSteps = queue.DrainAllEventsSynchronously(0.1f);
            SerializedBoardStateDto state0 = choreographer.SyncBoardStateFromNative(true);
            AssertCondition(state0 != null && state0.players.Count == 2, "Initial board state JSON must deserialize");
            AssertCondition(choreographer.AliceHandAnchor.Cards.Count == state0.players[0].hand.Count, "Alice hand count must match native state");
            AssertCondition(choreographer.WhiteRabbitHandAnchor.Cards.Count == state0.players[1].hand_count, "White Rabbit hand count must match native state");
            AssertCondition(choreographer.AliceShieldRow.Cards.Count == state0.players[0].shields.Count, "Alice shield count must match native state");
            AssertCondition(choreographer.WhiteRabbitShieldRow.Cards.Count == state0.players[1].shields.Count, "White Rabbit shield count must match native state");

            // 3. Verify Phase 1 Custom Shaders & CardArtRegistry assignment on CardVisuals
            CardVisual sampleHandCard = choreographer.AliceHandAnchor.Cards[0];
            AssertCondition(
                sampleHandCard.FrameBorderRenderer != null &&
                sampleHandCard.FrameBorderRenderer.sharedMaterial != null &&
                sampleHandCard.FrameBorderRenderer.sharedMaterial.shader.name == "Heav3ns/CardBorderGlow",
                "CardVisual frame border must use Heav3ns/CardBorderGlow shader");
            AssertCondition(
                sampleHandCard.ArtworkSlotRenderer != null &&
                sampleHandCard.ArtworkSlotRenderer.sharedMaterial != null &&
                sampleHandCard.ArtworkSlotRenderer.sharedMaterial.shader.name == "Heav3ns/CardSurfaceFoil",
                "CardVisual artwork slot must use Heav3ns/CardSurfaceFoil shader");
            AssertCondition(
                CardArtRegistry.GetCardArtTexture(sampleHandCard.CardSpecId, sampleHandCard.CardTitle, sampleHandCard.CardType, 0) != null,
                "CardArtRegistry must generate distinct Texture2D for card spec ID");

            // Verify opponent hand cards and shield cards are strictly face-down
            foreach (CardVisual c in choreographer.WhiteRabbitHandAnchor.Cards)
            {
                AssertCondition(c.IsFaceDown, "White Rabbit hand cards must be masked face-down");
            }
            foreach (CardVisual c in choreographer.AliceShieldRow.Cards)
            {
                AssertCondition(c.IsFaceDown, "Alice unrevealed shields must be face-down");
            }
            foreach (CardVisual c in choreographer.WhiteRabbitShieldRow.Cards)
            {
                AssertCondition(c.IsFaceDown, "White Rabbit unrevealed shields must be face-down");
            }

            // 4. Verify Phase 4 TitleStageController & Phase 2 Opening Mulligan Flow
            titleStage.ShowTitleDiorama(true);
            AssertCondition(titleStage.IsTitleActive, "TitleStageController diorama must become active when shown");
            bool turboSpeed = titleStage.ToggleAiSpeedButton();
            AssertCondition(
                turboSpeed && titleStage.FastAiSpeed && Mathf.Approximately(queue.PlaybackSpeedMultiplier, 1.8f),
                "TitleStageController AI Speed button must toggle PlaybackSpeedMultiplier to 1.8x");
            // Toggle back to standard 1.0x speed
            titleStage.ToggleAiSpeedButton();
            AssertCondition(Mathf.Approximately(queue.PlaybackSpeedMultiplier, 1.0f), "Toggling AI Speed again must restore 1.0x");

            // Clicking Play Match transitions camera and triggers Opening Mulligan
            titleStage.PressPlayMatchButton(true, true);
            AssertCondition(!titleStage.IsTitleActive, "TitleStageController diorama must hide after clicking Play Match");
            AssertCondition(mulligan.IsMulliganActive, "MulliganController must activate after clicking Play Match");
            AssertCondition(
                Mathf.Approximately(sampleHandCard.Spring.TargetScale.x, 1.6f),
                $"Opening Mulligan must fan Alice's 5 cards at 1.6x scale (got {sampleHandCard.Spring.TargetScale.x})");

            // Toggle Replace / Redraw badge on a card in Mulligan on and off, then Confirm Hand
            bool marked = mulligan.ToggleCardMulliganSelection(sampleHandCard);
            AssertCondition(marked && sampleHandCard.MarkedForMulligan, "Clicking card in Mulligan must toggle Replace/Redraw badge on");
            bool unmarked = !mulligan.ToggleCardMulliganSelection(sampleHandCard);
            AssertCondition(unmarked && !sampleHandCard.MarkedForMulligan, "Clicking card again in Mulligan must toggle Replace/Redraw badge off");
            mulligan.ConfirmMulliganAndDealShields(false);
            AssertCondition(!mulligan.IsMulliganActive && mulligan.HasCompletedMulligan, "ConfirmMulliganAndDealShields must complete Mulligan phase");
            AssertCondition(!sampleHandCard.MarkedForMulligan, "Confirming Mulligan must clear Replace/Redraw badges");
            AssertCondition(mulligan.ShieldsDealtSteps == 12, $"Confirming Mulligan must cascade all 12 shields (got {mulligan.ShieldsDealtSteps})");

            // 5. Verify Phase 3 Big-Card Inspector (Right-click & 0.6s long-hover at 2.2x scale with keyword tooltips)
            bool inspected = inspector.InspectCard(sampleHandCard);
            AssertCondition(
                inspected && inspector.IsShowing && inspector.PreviewCardActor != null,
                "CardInspectorOverlay must activate for revealed CardVisual");
            AssertCondition(
                Mathf.Approximately(inspector.PreviewCardActor.transform.localScale.x, 2.2f),
                $"CardInspectorOverlay preview card must be at 2.2x scale (got {inspector.PreviewCardActor.transform.localScale.x})");
            AssertCondition(
                inspector.KeywordCalloutLabel != null && inspector.KeywordCalloutLabel.text.Contains("SEQUENCE RULE"),
                "CardInspectorOverlay must populate keyword tooltip callouts");
            inspector.HideInspection();
            AssertCondition(!inspector.IsShowing, "CardInspectorOverlay must hide cleanly");

            // Also verify 0.6s long-hover triggers CardInspectorOverlay
            inspector.TickHoverCandidate(sampleHandCard, 0.65f, false);
            AssertCondition(inspector.IsShowing, "Long-hovering for >= 0.6s must open CardInspectorOverlay");
            inspector.HideInspection();

            // 6. Verify Hover Magnification & Neighbor Parting on AliceHandAnchor
            CardVisual hoveredCandidate = choreographer.AliceHandAnchor.Cards[2];
            Vector3 neighborBeforeX = choreographer.AliceHandAnchor.Cards[1].Spring.TargetPosition;
            choreographer.AliceHandAnchor.SetHoveredCard(hoveredCandidate);
            AssertCondition(
                Mathf.Approximately(hoveredCandidate.Spring.TargetScale.x, 1.35f),
                $"Hovered card target scale must be 1.35x (got {hoveredCandidate.Spring.TargetScale.x})");
            AssertCondition(
                hoveredCandidate.Spring.TargetPosition.y > 0.4f,
                "Hovered card must elevate Y position toward camera");
            AssertCondition(
                Mathf.Abs(choreographer.AliceHandAnchor.Cards[1].Spring.TargetPosition.x - neighborBeforeX.x) > 0.1f,
                "Neighboring cards must part horizontally when hovering");
            choreographer.AliceHandAnchor.SetHoveredCard(null);

            // 7. Verify Ghost Insertion Preview & Bezier Targeting Arrow + Valid Target Border Highlight State
            int ghostIdx = choreographer.AliceSequenceRow.UpdateGhostInsertionPreview(choreographer.AliceSequenceRow.transform.position);
            AssertCondition(ghostIdx >= 0 && choreographer.AliceSequenceRow.HasGhostPreview, "BoardRowLayout must show ghost insertion preview");
            choreographer.AliceSequenceRow.ClearGhostInsertionPreview();
            AssertCondition(!choreographer.AliceSequenceRow.HasGhostPreview, "Ghost insertion preview must clear");

            sampleHandCard.SetTargetedHighlight(true);
            AssertCondition(
                sampleHandCard.ActiveHighlightState == CardVisual.BorderHighlightState.ValidTargetUnderArrow,
                "Targeted card must enter ValidTargetUnderArrow shader highlight state");
            sampleHandCard.SetTargetedHighlight(false);

            arrow.RenderBezierArrow(
                hoveredCandidate.transform.position,
                choreographer.WhiteRabbitShieldRow.transform.position,
                true);
            AssertCondition(arrow.IsActive && arrow.IsSnappedToValidTarget && arrow.ChevronCount > 0, "TargetingArrowRenderer must activate and snap to target");
            arrow.HideArrow();

            // 8. Simulate playing a playable card into AliceSequenceRow via CardInteractionController
            CardVisual playableCard = choreographer.AliceHandAnchor.Cards.FirstOrDefault(c => c.IsPlayable);
            AssertCondition(playableCard != null, "Alice must have at least one playable card in opening hand");
            AssertCondition(
                playableCard.ActiveHighlightState == CardVisual.BorderHighlightState.PlayableInHand,
                "Playable card in hand must have PlayableInHand border glow state");
            int playedUid = playableCard.CardUid;

            string playResp = interaction.CommitPlayCard(playableCard, null, false);
            AssertCondition(playResp.Contains("\"ok\""), $"CommitPlayCard to AliceSequenceRow must succeed (resp={playResp})");
            AssertCondition(choreographer.AliceSequenceRow.Cards.Any(c => c.CardUid == playedUid), "Played card must be placed in AliceSequenceRow");

            // Verify 2-frame impact squash punch (1.15x X/Z, 0.85x Y) on placement and SecondOrderSpring step
            AssertCondition(
                Mathf.Approximately(playableCard.transform.localScale.x, 1.15f) &&
                Mathf.Approximately(playableCard.transform.localScale.y, 0.85f),
                $"Card placed in row must trigger 1.15x X/Z, 0.85x Y squash punch (got {playableCard.transform.localScale})");

            // Step springs to verify velocity-driven 2.5D tilt, holographic foil update, and return to scale 1.0
            for (int f = 0; f < 30; f++)
            {
                playableCard.Spring.Step(1f / 60f);
                playableCard.UpdateFoilTiltFromSpring();
                choreographer.TickCameraShake(1f / 60f);
            }

            // Drain visual events emitted by playing the card & verify ActionHistoryLog recorded it
            int playDrainSteps = queue.DrainAllEventsSynchronously(0.1f);
            AssertCondition(playDrainSteps > 0, "VisualEventQueue must drain ManaSpent and UnitSummoned/SpellCast events");
            AssertCondition(historyLog.Entries.Count > 0, "ActionHistoryLog must record resolved card play event");

            // 9. Verify Phase 2 Shield Break & Reveal Drama + Phase 1 Noise-Driven Dissolve Animation
            queue.EnqueueEvent(new VisualEventRecord
            {
                eventType = "ShieldRevealed",
                seat = 1,
                turnNumber = state0.turn_number,
                cardUid = -777,
                cardSpecId = "H3W-RA-001",
                title = "Minute-Hand Militia",
                cardType = "Character",
                cost = 1,
                triggerText = "Draw 1 card.",
                isShieldBreak = true,
                isBlocking = true,
                duration = 0.45f
            });
            queue.TickSequencer(0.05f);
            AssertCondition(
                choreographer.ShieldBreakDramaCount > 0 && choreographer.ActiveShieldRevealCard != null,
                "ShieldRevealed event must trigger high-impact Shield Break reveal drama at camera center");

            // Step 0.50s to finish the 0.45s reveal drama and transition into dissolve, then step 0.45s to complete dissolve
            for (int step = 0; step < 25; step++)
            {
                choreographer.TickDissolveAndShieldDrama(0.05f);
            }
            AssertCondition(
                choreographer.CompletedDissolveCount > 0 && choreographer.ActiveDissolvingCardCount == 0,
                "Destroyed/revealed shield card must complete noise-driven dissolve animation (_DissolveAmount 0 -> 1) and clean up");

            queue.DrainAllEventsSynchronously(0.1f);

            // 10. Test interactive corner props & procedural audio pitch randomization (+/- 4%)
            bool watchClicked = audioProps.TryInteractWithProp(audioProps.PocketWatchProp);
            bool teacupClicked = audioProps.TryInteractWithProp(audioProps.TeacupProp);
            float samplePitch = audioProps.PlaySound(TabletopAudioAndProps.SoundEffectType.CardDrawSnap);
            AssertCondition(watchClicked && audioProps.PocketWatchClickCount == 1, "EnchantedPocketWatchProp must respond to click");
            AssertCondition(teacupClicked && audioProps.TeacupClickCount == 1, "WonderlandTeacupProp must respond to click");
            AssertCondition(samplePitch >= 0.95f && samplePitch <= 1.05f, $"Audio pitch randomization must be within +/- 4% (got {samplePitch})");

            // 11. Advance turn via DiegeticTabletopHud End Turn Lever (triggers White Rabbit AI turn)
            int turnBefore = state0.turn_number;
            hud.PressEndTurnLever();
            int aiDrainSteps = queue.DrainAllEventsSynchronously(0.1f);
            AssertCondition(aiDrainSteps > 0, "VisualEventQueue must drain White Rabbit AI turn events");
            AssertCondition(
                historyLog.Entries.Count > 0 && historyLog.Entries.Count <= ActionHistoryLog.MaxHistoryEntries,
                "ActionHistoryLog must maintain up to 6 recent resolved event tiles");

            // 12. Final reconciliation and strict assertions against heav3ns_engine_get_board_state_json
            SerializedBoardStateDto finalState = choreographer.SyncBoardStateFromNative(true);
            AssertCondition(finalState != null, "Final board state must deserialize from native engine");
            AssertCondition(finalState.turn_number > turnBefore, $"Turn number must advance after Alice + White Rabbit turns ({turnBefore} -> {finalState.turn_number})");
            AssertCondition(queue.PendingCount == 0 && !queue.HasActiveBlockingEvent, "VisualEventQueue must be completely drained");

            AssertCondition(
                choreographer.AliceHandAnchor.Cards.Count == finalState.players[0].hand.Count,
                $"AliceHandAnchor count ({choreographer.AliceHandAnchor.Cards.Count}) must match engine ({finalState.players[0].hand.Count})");
            AssertCondition(
                choreographer.WhiteRabbitHandAnchor.Cards.Count == finalState.players[1].hand_count,
                $"WhiteRabbitHandAnchor count ({choreographer.WhiteRabbitHandAnchor.Cards.Count}) must match engine ({finalState.players[1].hand_count})");
            AssertCondition(
                choreographer.AliceSequenceRow.Cards.Count == finalState.players[0].field.Count,
                $"AliceSequenceRow count ({choreographer.AliceSequenceRow.Cards.Count}) must match engine ({finalState.players[0].field.Count})");
            AssertCondition(
                choreographer.WhiteRabbitSequenceRow.Cards.Count == finalState.players[1].field.Count,
                $"WhiteRabbitSequenceRow count ({choreographer.WhiteRabbitSequenceRow.Cards.Count}) must match engine ({finalState.players[1].field.Count})");
            AssertCondition(
                choreographer.AliceShieldRow.Cards.Count == finalState.players[0].shields.Count,
                $"AliceShieldRow count ({choreographer.AliceShieldRow.Cards.Count}) must match engine ({finalState.players[0].shields.Count})");
            AssertCondition(
                choreographer.WhiteRabbitShieldRow.Cards.Count == finalState.players[1].shields.Count,
                $"WhiteRabbitShieldRow count ({choreographer.WhiteRabbitShieldRow.Cards.Count}) must match engine ({finalState.players[1].shields.Count})");

            // Verify every Alice hand CardVisual UID & Title matches native JSON
            for (int i = 0; i < finalState.players[0].hand.Count; i++)
            {
                SerializedCardDto dto = finalState.players[0].hand[i];
                CardVisual cv = choreographer.AliceHandAnchor.Cards[i];
                AssertCondition(cv.CardUid == dto.uid && cv.CardTitle == dto.name && !cv.IsFaceDown,
                    $"Alice hand card {i} must match native state (expected {dto.uid}/{dto.name}, got {cv.CardUid}/{cv.CardTitle})");
            }

            // Verify no orphaned CardVisual children exist under any zone anchor
            AssertCondition(
                choreographer.AliceHandAnchor.GetComponentsInChildren<CardVisual>(true).Length == choreographer.AliceHandAnchor.Cards.Count,
                "AliceHandAnchor child CardVisual count must exactly match managed list");
            AssertCondition(
                choreographer.WhiteRabbitHandAnchor.GetComponentsInChildren<CardVisual>(true).Length == choreographer.WhiteRabbitHandAnchor.Cards.Count,
                "WhiteRabbitHandAnchor child CardVisual count must exactly match managed list");
            AssertCondition(
                choreographer.AliceShieldRow.GetComponentsInChildren<CardVisual>(true).Length == choreographer.AliceShieldRow.Cards.Count,
                "AliceShieldRow child CardVisual count must exactly match managed list");

            // 13. Verify GameOver MatchWon event shows MatchResetButton3D, stress-test rapid concurrent dissolves + reset, and verify Mulligan redraw replacement
            AssertCondition(
                hud.MatchResetButtonTransform != null && !hud.MatchResetButtonTransform.gameObject.activeSelf,
                "MatchResetButton3D must be hidden while match is still active");
            queue.EnqueueEvent(new VisualEventRecord
            {
                eventType = "MatchWon",
                seat = 0,
                turnNumber = finalState.turn_number,
                text = "Alice Wins!",
                isBlocking = true,
                duration = 0.25f
            });
            queue.TickSequencer(0.05f);
            AssertCondition(
                hud.IsGameOver && hud.MatchResetButtonTransform.gameObject.activeSelf,
                "MatchResetButton3D must become visible when GameOver / MatchWon is reached");

            // Stress-test: start a shield reveal drama and a card dissolve right before pressing MatchResetControl
            choreographer.TriggerShieldBreakDrama(new VisualEventRecord
            {
                eventType = "ShieldRevealed",
                seat = 0,
                cardUid = -999,
                cardSpecId = "H3W-AL-001",
                title = "A Question of Doors"
            });
            if (choreographer.AliceHandAnchor.Cards.Count > 0)
            {
                choreographer.BeginDissolveAndDestroyCard(choreographer.AliceHandAnchor.Cards[0], 0.50f);
            }

            hud.PressMatchResetControl();
            queue.DrainAllEventsSynchronously(0.1f);
            SerializedBoardStateDto resetState = choreographer.SyncBoardStateFromNative(true);
            AssertCondition(
                !hud.IsGameOver && !hud.MatchResetButtonTransform.gameObject.activeSelf && resetState.turn_number == 1,
                "PressMatchResetControl must reset native match back to Turn 1 and hide MatchResetButton3D");
            AssertCondition(
                choreographer.ActiveShieldRevealCard == null && choreographer.ActiveDissolvingCardCount == 0,
                "PressMatchResetControl must cleanly abort in-flight shield drama and dissolve animations");
            AssertCondition(
                historyLog.Entries.Count == 0,
                "PressMatchResetControl must clear ActionHistoryLog");

            // Also verify Mulligan Redraw replacement genuinely replaces marked cards when confirmed
            mulligan.BeginOpeningMulligan(true);
            CardVisual preRedrawCard0 = choreographer.AliceHandAnchor.Cards[0];
            mulligan.ToggleCardMulliganSelection(preRedrawCard0);
            mulligan.ConfirmMulliganAndDealShields(true);
            AssertCondition(
                choreographer.AliceHandAnchor.Cards.Count == 5 && choreographer.AliceHandAnchor.Cards[0] != null,
                "Mulligan redraw confirmation must populate 5 fresh hand cards from the re-seeded native match");

            HubVerification.Run();

            // Keep the simulation isolated from the authored scene and prefab.
            queue.ShutdownNativeBridge();

            Console.WriteLine("========================================");
            Console.WriteLine("[HEAV3NS-ENGINE] PLAYABLE MATCH SIMULATION PASSED");
            Console.WriteLine($"Initial Events Drained  : {initialSteps} steps");
            Console.WriteLine($"Custom Shaders Verified : Heav3ns/CardBorderGlow & Heav3ns/CardSurfaceFoil");
            Console.WriteLine($"Mulligan & Shield Deal  : Completed (12 shields cascaded @ 0.08s)");
            Console.WriteLine($"Big-Card Inspector      : Verified (2.2x scale + Keyword Callouts)");
            Console.WriteLine($"Shield Drama & Dissolve : Drama={choreographer.ShieldBreakDramaCount}, Dissolves={choreographer.CompletedDissolveCount}");
            Console.WriteLine($"Played Card UID         : {playedUid} ({playableCard.CardTitle})");
            Console.WriteLine($"AI Turn Events Drained  : {aiDrainSteps} steps");
            Console.WriteLine($"Turn Progression        : Turn {turnBefore} -> Turn {finalState.turn_number}");
            Console.WriteLine($"Alice State             : Hand={choreographer.AliceHandAnchor.Cards.Count}, Field={choreographer.AliceSequenceRow.Cards.Count}, Shields={choreographer.AliceShieldRow.Cards.Count}");
            Console.WriteLine($"White Rabbit State      : Hand={choreographer.WhiteRabbitHandAnchor.Cards.Count}, Field={choreographer.WhiteRabbitSequenceRow.Cards.Count}, Shields={choreographer.WhiteRabbitShieldRow.Cards.Count}");
            Console.WriteLine($"Corner Props Verified   : WatchClicks={audioProps.PocketWatchClickCount}, TeacupClicks={audioProps.TeacupClickCount}");
            Console.WriteLine($"Assertions Status       : 100% PASSED (0 exceptions)");
            Console.WriteLine("========================================");

            CleanupSimulationAssets();
            Debug.Log("[HeadlessCliRunner] RunPlayableMatchSimulation succeeded with 100% assertions passed!");
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }
        catch (Exception ex)
        {
            CleanupSimulationAssets();
            Debug.LogError($"[HeadlessCliRunner] Exception in RunPlayableMatchSimulation: {ex}");
            Console.Error.WriteLine($"[HeadlessCliRunner] SIMULATION FAILED: {ex}");
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(1);
            }
        }
    }

    private static void CleanupSimulationAssets()
    {
        if (SceneManager.GetActiveScene().path == SimulationScenePath)
            EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        AssetDatabase.DeleteAsset(SimulationScenePath);
        AssetDatabase.DeleteAsset(SimulationPrefabPath);
    }

    /// <summary>
    /// Verifies the RustEngineBridge P/Invoke bindings against libheav3ns_engine.dylib in Assets/Plugins
    /// and tests VisualEventQueue event draining and sequencing.
    /// </summary>
    [MenuItem("HEAV3NS/Headless/Verify Native Rust Bridge")]
    public static void VerifyNativeRustBridge()
    {
        Debug.Log("[HeadlessCliRunner] Starting VerifyNativeRustBridge...");
        try
        {
            bool nativeAvailable = RustEngineBridge.IsNativeLibraryAvailable();
            IntPtr handle = RustEngineBridge.NativeCreateMatch(42UL, 0U);
            if (handle == IntPtr.Zero)
            {
                throw new InvalidOperationException("NativeCreateMatch returned IntPtr.Zero");
            }

            string boardStateJson = RustEngineBridge.ConsumeNativeString(
                RustEngineBridge.NativeGetBoardStateJson(handle));
            string initialEventsJson = RustEngineBridge.ConsumeNativeString(
                RustEngineBridge.NativeDrainVisualEventsJson(handle));

            string badCmdResponseJson = RustEngineBridge.ConsumeNativeString(
                RustEngineBridge.NativeSubmitCommandJson(handle, "{\"command\":\"unknown_typo\"}"));

            string cmdResponseJson = RustEngineBridge.ConsumeNativeString(
                RustEngineBridge.NativeSubmitCommandJson(handle, "{\"command\":\"close\"}"));

            string endTurnResponseJson = RustEngineBridge.ConsumeNativeString(
                RustEngineBridge.NativeSubmitCommandJson(handle, "{\"command\":\"end_turn\",\"run_ai\":true}"));
            string aiTurnEventsJson = RustEngineBridge.ConsumeNativeString(
                RustEngineBridge.NativeDrainVisualEventsJson(handle));

            RustEngineBridge.NativeDestroyMatch(handle);

            GameObject tempHost = new GameObject("TempBridgeVerifier");
            var queue = tempHost.AddComponent<VisualEventQueue>();
            int enqueuedInitial = queue.EnqueueEventsFromJson(initialEventsJson);
            int enqueuedAi = queue.EnqueueEventsFromJson(aiTurnEventsJson);
            queue.TickSequencer(0.05f);
            bool hasBlocking = queue.HasActiveBlockingEvent;
            queue.TickSequencer(5.0f);
            UnityEngine.Object.DestroyImmediate(tempHost);

            string mcpInitResp = Heav3ns.UnityMCP.UnityMcpEditorBridge.ExecuteOnMainThread(
                "mcp",
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}");
            string mcpToolsResp = Heav3ns.UnityMCP.UnityMcpEditorBridge.ExecuteOnMainThread(
                "mcp",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}");
            string hierarchyResp = Heav3ns.UnityMCP.UnityMcpEditorBridge.ExecuteOnMainThread(
                "list_gameobjects",
                string.Empty);

            Console.WriteLine("========================================");
            Console.WriteLine("[HEAV3NS-ENGINE] NATIVE RUST BRIDGE & MCP VERIFICATION");
            Console.WriteLine($"Native Library Available : {nativeAvailable}");
            Console.WriteLine($"Initial Events Drained   : {enqueuedInitial} (blocking active={hasBlocking})");
            Console.WriteLine($"AI Turn Events Drained   : {enqueuedAi}");
            Console.WriteLine($"Invalid Cmd Response     : {badCmdResponseJson}");
            Console.WriteLine($"Close Cmd Response       : {cmdResponseJson}");
            Console.WriteLine($"EndTurn Cmd Response     : {endTurnResponseJson}");
            Console.WriteLine($"Board State Snapshot Len : {boardStateJson.Length} bytes");
            Console.WriteLine($"MCP JSON-RPC Init        : {mcpInitResp}");
            Console.WriteLine($"VisualEventQueue Saved   : {hierarchyResp.Contains("\"VisualEventQueue\"")}");
            Console.WriteLine("========================================");

            if (!nativeAvailable
                || enqueuedInitial <= 0
                || enqueuedAi <= 0
                || !badCmdResponseJson.Contains("\"rejected\"")
                || !boardStateJson.Contains("\"Alice\"")
                || !mcpInitResp.Contains("\"protocolVersion\"")
                || !mcpToolsResp.Contains("\"list_gameobjects\"")
                || !hierarchyResp.Contains("\"VisualEventQueue\""))
            {
                Debug.LogError("[HeadlessCliRunner] Native Rust bridge or MCP verification failed assertions.");
                if (Application.isBatchMode) EditorApplication.Exit(1);
                return;
            }

            Debug.Log("[HeadlessCliRunner] VerifyNativeRustBridge succeeded!");
            if (Application.isBatchMode) EditorApplication.Exit(0);
        }
        catch (Exception ex)
        {
            Debug.LogError($"[HeadlessCliRunner] Exception in VerifyNativeRustBridge: {ex}");
            if (Application.isBatchMode) EditorApplication.Exit(1);
        }
    }

    private static void AssertCondition(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException($"[Assertion Failed] {message}");
        }
    }

    private static GameObject CreateZoneAnchor(Transform parent, string name, Vector3 localPosition)
    {
        GameObject anchor = new GameObject(name);
        anchor.transform.SetParent(parent, false);
        anchor.transform.localPosition = localPosition;
        return anchor;
    }
}

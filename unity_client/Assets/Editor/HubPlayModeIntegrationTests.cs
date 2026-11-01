using System.Collections;
using System.Reflection;
using Heav3ns.Choreography;
using Heav3ns.UI;
using Heav3ns.UI.Navigation;
using NUnit.Framework;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.TestTools;

namespace Heav3ns.Tests.Editor
{
    public class HubPlayModeIntegrationTests
    {
        [UnityTest]
        public IEnumerator HubToBattleAndBackUsesTheLiveScene()
        {
            EditorSceneManager.OpenScene("Assets/Scenes/Battlefield.unity");
            yield return new EnterPlayMode();
            yield return null;
            var router = Object.FindAnyObjectByType<HubNavigationRouter>();
            var hub = Object.FindAnyObjectByType<GachaHubLobbyController>();
            Assert.IsNotNull(router); Assert.IsNotNull(hub);
            Assert.IsTrue(router.NavigateTo(HubScreenState.HubLobby));
            Assert.IsFalse(router.NavigateTo(HubScreenState.GachaSummonAltar));
            router.AdvanceTransition(1);
            Assert.IsTrue(router.NavigateTo(HubScreenState.GachaSummonAltar)); router.AdvanceTransition(1);
            var summon = Object.FindAnyObjectByType<GachaSummonStage>();
            var catalog = JsonUtility.FromJson<HubCardCatalog>(Resources.Load<TextAsset>("Hub/CardCatalog").text);
            var progress = new HubProgress(new HubSaveData { pullsSinceSsr = 9 }, _ => { }, max => max - 1);
            progress.EnsureStarterDecks(catalog.cards);
            typeof(GachaHubLobbyController).GetField("_progress", BindingFlags.Instance | BindingFlags.NonPublic).SetValue(hub, progress);
            var preBattle = Object.FindAnyObjectByType<PreBattleDrawerController>();
            typeof(PreBattleDrawerController).GetField("_progress", BindingFlags.Instance | BindingFlags.NonPublic).SetValue(preBattle, progress);
            summon.Initialize(progress, catalog.cards);
            Assert.IsTrue(summon.PurchasePack()); Assert.IsTrue(summon.Unseal());
            Assert.AreEqual(SummonRarity.Ssr, summon.HoverRarity(0));
            summon.RevealAll();
            float revealDeadline = Time.realtimeSinceStartup + 8f;
            while (!summon.IsComplete && Time.realtimeSinceStartup < revealDeadline) yield return null;
            Assert.IsTrue(summon.IsComplete);
            Assert.IsTrue(router.NavigateBack()); router.AdvanceTransition(1);
            Assert.IsTrue(router.NavigateTo(HubScreenState.GrimoireStudio)); router.AdvanceTransition(1);
            var studio = Object.FindAnyObjectByType<GrimoireDeckStudio>();
            studio.SetFaction(GrimoireFaction.Alice); studio.SetCost(1);
            Assert.IsNotEmpty(studio.FilteredCards);
            typeof(GachaHubLobbyController).GetMethod("Inspect", BindingFlags.Instance | BindingFlags.NonPublic).Invoke(hub, new object[] { studio.FilteredCards[0] });
            Assert.IsTrue(Object.FindAnyObjectByType<CardInspectorOverlay>().IsShowing);
            Assert.IsTrue(router.NavigateBack()); router.AdvanceTransition(1);
            Assert.IsTrue(router.NavigateTo(HubScreenState.PreBattleDrawer)); router.AdvanceTransition(1);
            Assert.IsTrue(hub.CommenceDuel(), "A valid saved deck must reach the native match");
            var mulligan = Object.FindAnyObjectByType<MulliganController>();
            Assert.IsTrue(mulligan.IsMulliganActive); Assert.AreEqual(5, mulligan.MulliganCards.Count);
            router.AdvanceTransition(1);
            Assert.IsTrue(router.NavigateBack()); Assert.IsTrue(router.IsPauseOpen);
            Assert.IsTrue(hub.ReturnToLobby()); Assert.AreEqual(HubScreenState.HubLobby, router.Current);
            Assert.IsFalse(mulligan.IsMulliganActive); Assert.AreEqual(1f, Time.timeScale);
            yield return new ExitPlayMode();
        }
    }
}

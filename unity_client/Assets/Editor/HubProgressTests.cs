using System.Collections.Generic;
using Heav3ns.UI;
using Heav3ns.UI.Navigation;
using Heav3ns.UnityBridge;
using Heav3ns.Choreography;
using NUnit.Framework;
using UnityEngine;

namespace Heav3ns.Tests.Editor
{
    public class HubProgressTests
    {
        [Test] public void HubCommissionModelsRunTogether() { HubVerification.Run(); }
        private List<HubCardDefinition> _cards;
        [SetUp] public void SetUp()
        {
            _cards = new List<HubCardDefinition> {
                new HubCardDefinition { id = "H3W-AL-008", title = "Checkmate the Sky", deck = "Alice", cost = 3, copies = 2 },
                new HubCardDefinition { id = "rabbit", title = "Rabbit", deck = "White Rabbit", cost = 6, copies = 3 }
            };
        }
        [Test] public void SummonDeductsOncePersistsAndGuaranteesPity()
        {
            var data = new HubSaveData { keys = 1, quartz = 320, pullsSinceSsr = 9 };
            int saves = 0;
            var progress = new HubProgress(data, _ => saves++, max => max - 1);
            Assert.IsTrue(progress.TrySummon(_cards, out var cards));
            Assert.AreEqual(5, cards.Count);
            Assert.AreEqual(SummonRarity.Ssr, cards[0].Rarity);
            Assert.AreEqual("H3W-AL-008", cards[0].Card.id);
            Assert.AreEqual(0, data.keys); Assert.AreEqual(320, data.quartz);
            Assert.AreEqual(0, data.pullsSinceSsr); Assert.AreEqual(5, data.ownedCardIds.Count); Assert.AreEqual(1, saves);
            Assert.IsTrue(progress.TrySummon(_cards, out _));
            Assert.AreEqual(160, data.quartz); Assert.AreEqual(1, data.pullsSinceSsr);
            data.quartz = 0;
            Assert.IsFalse(progress.TrySummon(_cards, out _)); Assert.AreEqual(2, saves);
        }
        [Test] public void TeaRegenerationUsesElapsedTimeAndDoesNotBankAboveCap()
        {
            var data = new HubSaveData { tea = 118, teaUpdatedAt = 1000 };
            var progress = new HubProgress(data, _ => { }, max => 0);
            progress.RefreshTea(1301); Assert.AreEqual(119, data.tea); Assert.AreEqual(1300, data.teaUpdatedAt);
            progress.RefreshTea(2000); Assert.AreEqual(120, data.tea); Assert.AreEqual(2000, data.teaUpdatedAt);
            progress.RefreshTea(3000); Assert.AreEqual(3000, data.teaUpdatedAt);
        }
        [Test] public void DailyRewardCanBeClaimedOnlyOnceAfterBothSteps()
        {
            var data = new HubSaveData(); var progress = new HubProgress(data, _ => { }, max => max - 1);
            progress.RefreshDaily("2026-11-01");
            progress.RecordGrimoireVisit(); Assert.IsFalse(progress.ClaimDaily());
            Assert.IsTrue(progress.TrySummon(_cards, out _));
            Assert.IsTrue(progress.ClaimDaily()); Assert.AreEqual(10, data.keys); Assert.AreEqual(4960, data.quartz);
            Assert.IsFalse(progress.ClaimDaily());
            progress.RefreshDaily("2026-11-02"); Assert.IsFalse(data.dailyClaimed); Assert.IsFalse(data.dailySummoned);
        }
        [Test] public void RevealAndCollectionFiltersHaveStableState()
        {
            var go = new GameObject("hub tests");
            try
            {
                var progress = new HubProgress(new HubSaveData(), _ => { }, max => max - 1);
                var summon = go.AddComponent<GachaSummonStage>(); summon.Initialize(progress, _cards);
                Assert.IsTrue(summon.PurchasePack()); Assert.IsFalse(summon.PurchasePack());
                Assert.IsTrue(summon.Unseal()); Assert.IsNotNull(summon.HoverRarity(0));
                Assert.IsTrue(summon.Reveal(0)); Assert.IsNull(summon.HoverRarity(0));
                Assert.IsFalse(summon.Reveal(0)); summon.RevealAllImmediately(); Assert.IsTrue(summon.IsComplete);
                Assert.IsTrue(summon.Continue());
                var studio = go.AddComponent<GrimoireDeckStudio>(); studio.Initialize(_cards);
                studio.SetFaction(GrimoireFaction.WhiteRabbit); studio.SetCost(6);
                Assert.AreEqual(1, studio.FilteredCards.Count); Assert.AreEqual("rabbit", studio.FilteredCards[0].id);
                Assert.AreEqual(3, studio.ManaCurve(GrimoireFaction.WhiteRabbit)[5]);
                studio.SetCost(1); Assert.AreEqual(0, studio.FilteredCards.Count);
            }
            finally { Object.DestroyImmediate(go); }
        }
        [Test] public void DeckEditingAndNativeMatchUseTheSameThirtyCards()
        {
            var catalog = JsonUtility.FromJson<HubCardCatalog>(Resources.Load<TextAsset>("Hub/CardCatalog").text);
            var data = new HubSaveData();
            var progress = new HubProgress(data, _ => { }, max => max - 1);
            progress.EnsureStarterDecks(catalog.cards);
            Assert.AreEqual(30, progress.ActiveDeck.Count);
            string id = progress.ActiveDeck[0];
            Assert.IsTrue(progress.RemoveFromDeck(id));
            Assert.AreEqual(29, progress.ActiveDeck.Count);
            HubCardDefinition card = System.Array.Find(catalog.cards, item => item.id == id);
            Assert.IsTrue(progress.AddToDeck(card));
            progress.SetLeader(true);
            var host = new GameObject("Native custom deck test");
            try
            {
                var queue = host.AddComponent<VisualEventQueue>();
                Assert.IsTrue(queue.StartCustomDeck(progress.ActiveDeck, 45, 3));
                var state = JsonUtility.FromJson<SerializedBoardStateDto>(queue.GetBoardStateJson());
                Assert.AreEqual("White Rabbit", state.players[0].name);
                Assert.AreEqual(19, state.players[0].deck_count);
                Assert.AreEqual(5, state.players[0].hand.Count);
                Assert.IsTrue(queue.ReseedAndReinitializeBridge());
                state = JsonUtility.FromJson<SerializedBoardStateDto>(queue.GetBoardStateJson());
                Assert.AreEqual("White Rabbit", state.players[0].name);
                Assert.AreEqual(19, state.players[0].deck_count);
                queue.ShutdownNativeBridge();
            }
            finally { Object.DestroyImmediate(host); }
        }
    }
}

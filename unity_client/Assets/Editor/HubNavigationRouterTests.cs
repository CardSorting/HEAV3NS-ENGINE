using Heav3ns.UI.Navigation;
using NUnit.Framework;
using UnityEngine;

namespace Heav3ns.Tests.Editor
{
    public class HubNavigationRouterTests
    {
        private GameObject _host;
        private HubNavigationRouter _router;

        [SetUp] public void SetUp() { _host = new GameObject("Navigation test"); _router = _host.AddComponent<HubNavigationRouter>(); }
        [TearDown] public void TearDown() { Object.DestroyImmediate(_host); }
        private void Settle() { _router.AdvanceTransition(1f); }

        [Test] public void NavigationIsAtomicAndBackReturnsToLobby()
        {
            Assert.IsFalse(_router.NavigateTo(HubScreenState.ActiveBattlefield));
            Assert.IsTrue(_router.NavigateTo(HubScreenState.HubLobby));
            Assert.IsTrue(_router.IsTransitioning);
            Assert.IsFalse(_router.NavigateTo(HubScreenState.GachaSummonAltar));
            Assert.IsFalse(_router.NavigateBack());
            Settle();
            Assert.IsTrue(_router.NavigateTo(HubScreenState.GachaSummonAltar)); Settle();
            Assert.IsTrue(_router.HandleBackShortcut(true, false)); Settle();
            Assert.AreEqual(HubScreenState.HubLobby, _router.Current);
            Assert.IsTrue(_router.NavigateTo(HubScreenState.GrimoireStudio)); Settle();
            Assert.IsTrue(_router.NavigateBack()); Settle();
            Assert.IsTrue(_router.NavigateTo(HubScreenState.PreBattleDrawer)); Settle();
            Assert.IsTrue(_router.NavigateTo(HubScreenState.ActiveBattlefield)); Settle();
            Assert.IsTrue(_router.NavigateBack());
            Assert.IsTrue(_router.IsPauseOpen);
            Assert.IsFalse(_router.NavigateHome());
            Assert.IsTrue(_router.ReturnFromBattle());
            Assert.AreEqual(HubScreenState.HubLobby, _router.Current);
            Assert.IsFalse(_router.IsPauseOpen);
        }

        [Test] public void HomeClearsHistoryAndDoesNotReturnToSplash()
        {
            Assert.IsTrue(_router.NavigateTo(HubScreenState.HubLobby)); Settle();
            Assert.IsTrue(_router.NavigateTo(HubScreenState.GrimoireStudio)); Settle();
            Assert.IsTrue(_router.NavigateHome()); Settle();
            Assert.AreEqual(0, _router.HistoryCount);
            Assert.IsFalse(_router.NavigateBack());
        }

        [Test] public void HeaderShortcutFromSubscreenReturnsToLobby()
        {
            Assert.IsTrue(_router.NavigateTo(HubScreenState.HubLobby)); Settle();
            Assert.IsTrue(_router.NavigateTo(HubScreenState.GrimoireStudio)); Settle();
            Assert.IsTrue(_router.OpenSummonFromHeader()); Settle();
            Assert.IsTrue(_router.NavigateBack()); Settle();
            Assert.AreEqual(HubScreenState.HubLobby, _router.Current);
        }
    }
}

import { setSearchQuery, getCurrentView, getCurrentAlbumContext } from "./state";
import { loadTracks, renderAlbumDetails } from "./views";

export function setupSearch() {
  const searchOverlay = document.getElementById("search-overlay");
  const searchInput = document.getElementById("search-input") as HTMLInputElement;

  document.addEventListener("keydown", (e) => {
    if (!searchOverlay?.classList.contains("hidden")) return;
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      searchOverlay?.classList.remove("hidden");
      if (searchInput) {
        searchInput.value = e.key;
        searchInput.focus();
        setSearchQuery(e.key.toLowerCase());
        refreshCurrentView();
      }
    }
  });

  if (searchInput) {
    searchInput.addEventListener("input", (e) => {
      setSearchQuery((e.target as HTMLInputElement).value.toLowerCase().trim());
      refreshCurrentView();
    });

    searchInput.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        searchInput.value = "";
        setSearchQuery("");
        searchOverlay?.classList.add("hidden");
        refreshCurrentView();
      }
    });
  }
}

function refreshCurrentView() {
  const view = getCurrentView();
  if (view === 'tracks') loadTracks();
  if (view === 'album-details' && getCurrentAlbumContext()) renderAlbumDetails(getCurrentAlbumContext()!);
}

export function resetContentScroll(container: HTMLDivElement | null) {
  if (!container) return;

  container.scrollTo({
    behavior: "auto",
    top: 0,
  });
}

export function scrollHighlightedSetting(
  container: HTMLDivElement | null,
  settingId: string,
  reduceMotion: boolean,
) {
  if (!container) return;

  const target = container.querySelector<HTMLElement>(
    `[data-preference-setting-id="${settingId}"]`,
  );
  if (!target) return;

  target.scrollIntoView({
    behavior: reduceMotion ? "auto" : "smooth",
    block: "center",
  });
}

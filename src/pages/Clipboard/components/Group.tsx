import { useMount } from "ahooks";
import type { TFunction } from "i18next";
import type {
  Dispatch,
  FC,
  MouseEvent,
  RefObject,
  SetStateAction,
} from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";
import {
  createClipboardGroup,
  deleteClipboardGroup,
  listClipboardGroups,
  openPreferenceWithHighlight,
  updateClipboardGroup,
} from "@/commands";
import ClipboardGroupIcon from "@/components/ClipboardGroupIcon";
import ClipboardGroupModal from "@/components/ClipboardGroupModal";
import Dropdown, { type DropdownMenuItems } from "@/components/Dropdown";
import KeyHint from "@/components/KeyHint";
import Tooltip from "@/components/Tooltip";
import { TAURI_EVENT } from "@/constants/events";
import { useKeyboardEvent } from "@/hooks/useKeyboardEvent";
import { useTauriListen } from "@/hooks/useTauriListen";
import { clipboardViewState } from "@/stores/clipboardView";
import type {
  ClipboardCategory,
  ClipboardGroupIcon as ClipboardGroupIconValue,
  ClipboardGroupInput,
  ClipboardGroupRecord,
  ClipboardRange,
} from "@/types/clipboard";
import { cn } from "@/utils/cn";
import { getModalApi } from "@/utils/feedback";
import { resolveFilterSelectedClass } from "../itemTone";

type GroupModalMode = "create" | "edit";
type MoreMenuAction = "manageGroups" | "newGroup";
type GroupMenuAction = "delete" | "edit" | "hide";
type MoreMenuGroupKey = `group:${string}`;

interface RangeGroupOption {
  labelKey: string;
  value: ClipboardRange;
  icon: ClipboardGroupIconValue;
}

interface CategoryGroupOption {
  labelKey: string;
  value: ClipboardCategory;
  icon: ClipboardGroupIconValue;
}

interface OverflowGroupMenuLabelProps {
  menuItems: DropdownMenuItems;
  onContext: (record: ClipboardGroupRecord) => void;
  onMenuClick: (info: { key: string }) => void;
  record: ClipboardGroupRecord;
}

interface GroupSeparatorProps {
  separatorRef?: RefObject<HTMLSpanElement | null>;
}

const RANGE_GROUP_OPTIONS: RangeGroupOption[] = [
  { icon: "i-lets-icons:widget", labelKey: "groups.all", value: "all" },
  {
    icon: "i-lets-icons:star",
    labelKey: "groups.favorite",
    value: "favorite",
  },
];

const CATEGORY_GROUP_OPTIONS: CategoryGroupOption[] = [
  { icon: "i-lets-icons:file-dock", labelKey: "groups.text", value: "text" },
  { icon: "i-lets-icons:img-box", labelKey: "groups.image", value: "image" },
  {
    icon: "i-lets-icons:folder-file-alt",
    labelKey: "groups.files",
    value: "files",
  },
];

const GROUP_MENU_ACTION = {
  DELETE: "delete",
  EDIT: "edit",
  HIDE: "hide",
} as const satisfies Record<string, GroupMenuAction>;

const MORE_MENU_ACTION = {
  MANAGE_GROUPS: "manageGroups",
  NEW_GROUP: "newGroup",
} as const satisfies Record<string, MoreMenuAction>;

const CUSTOM_GROUPS_SETTING_ID = "organizing.customGroups";

const GROUP_BUTTON_BASE_CLASS =
  "flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-1.5 border-0 bg-transparent p-0 transition-colors";
const GROUP_ICON_BUTTON_CLASS = GROUP_BUTTON_BASE_CLASS;
const GROUP_BUTTON_WIDTH = 24;
const GROUP_BUTTON_GAP = 4;
const GROUP_SEPARATOR_MARGIN = 4;

/** Group filter bar with built-in categories and custom group entry points. */
const Group: FC = () => {
  const { t } = useTranslation(["clipboard", "common"]);
  const { category, groupId, range } = useSnapshot(clipboardViewState);

  const [customGroups, setCustomGroups] = useState<ClipboardGroupRecord[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<GroupModalMode>("create");
  const [visibleCustomGroupCount, setVisibleCustomGroupCount] = useState(
    Number.POSITIVE_INFINITY,
  );
  const [editingGroup, setEditingGroup] = useState<ClipboardGroupRecord | null>(
    null,
  );
  const toolbarRef = useRef<HTMLDivElement>(null);
  const customGroupAnchorRef = useRef<HTMLSpanElement>(null);
  const contextGroupRef = useRef<ClipboardGroupRecord | null>(null);
  const deleteGroupRef = useRef<ClipboardGroupRecord | null>(null);

  const visibleCustomGroups = customGroups.filter((record) => {
    return !record.isHidden;
  });
  const inlineCustomGroups = visibleCustomGroups.slice(
    0,
    visibleCustomGroupCount,
  );
  const overflowCustomGroups = visibleCustomGroups.slice(
    visibleCustomGroupCount,
  );

  /** Load custom groups from Rust. */
  const loadGroups = async () => {
    const groups = await listClipboardGroups();

    setCustomGroups(groups);
    scheduleVisibleCustomGroupCountUpdate(
      toolbarRef,
      customGroupAnchorRef,
      setVisibleCustomGroupCount,
      groups.filter((record) => {
        return !record.isHidden;
      }).length,
    );
    ensureSelectedGroupStillExists(groups);
  };

  useMount(() => {
    void loadGroups();
  });

  /** Refresh the local list after another window or command changes groups. */
  const handleGroupsUpdated = () => {
    void loadGroups();
  };

  useTauriListen(TAURI_EVENT.CLIPBOARD_GROUPS_UPDATED, handleGroupsUpdated);

  /** Recalculate overflow when the toolbar size changes. */
  useEffect(() => {
    const toolbar = toolbarRef.current;
    const customAnchor = customGroupAnchorRef.current;
    if (!toolbar || !customAnchor) return;

    const updateVisibleCustomGroupCount = () => {
      commitVisibleCustomGroupCount(
        toolbar,
        customAnchor,
        setVisibleCustomGroupCount,
        visibleCustomGroups.length,
      );
    };

    const observer = new ResizeObserver(updateVisibleCustomGroupCount);
    observer.observe(toolbar);
    updateVisibleCustomGroupCount();

    return () => {
      observer.disconnect();
    };
  }, [visibleCustomGroups.length]);

  /** Switch range while keeping one range selected. */
  const selectRange = (value: ClipboardRange) => {
    clipboardViewState.range = value;
    clipboardViewState.category = null;
  };

  /**
   * 종류 탭은 즐겨찾기와 별개다. 켜면 해당 종류 전체를 보고, 같은 탭을 다시 누르면 해제한다.
   */
  const toggleCategory = (value: ClipboardCategory) => {
    clipboardViewState.range = "all";
    clipboardViewState.category =
      clipboardViewState.category === value ? null : value;
  };

  /** Toggle a custom group, clearing it when selected again. */
  const toggleCustomGroup = (id: string) => {
    clipboardViewState.groupId = clipboardViewState.groupId === id ? null : id;
  };

  const handleGroupClick = (event: MouseEvent<HTMLButtonElement>) => {
    const type = event.currentTarget.dataset.type;
    const value = event.currentTarget.dataset.value;
    const nextGroupId = event.currentTarget.dataset.groupId;

    if (nextGroupId) {
      toggleCustomGroup(nextGroupId);
      return;
    }

    if (type === "range" && isRangeGroup(value)) {
      selectRange(value);
      return;
    }

    if (type === "category" && isCategoryGroup(value)) {
      toggleCategory(value);
    }
  };

  const handleCustomGroupContextMenu = (
    event: MouseEvent<HTMLButtonElement>,
  ) => {
    const nextGroupId = event.currentTarget.dataset.groupId;
    if (!nextGroupId) return;

    contextGroupRef.current =
      customGroups.find((record) => {
        return record.id === nextGroupId;
      }) ?? null;
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    const eventModifierPressed = event.metaKey || event.ctrlKey;

    if (eventModifierPressed && event.key.toLowerCase() === "q") {
      event.preventDefault();
      toggleRange();

      return;
    }

    if (
      (event.key === "ArrowLeft" || event.key === "ArrowRight") &&
      !shouldUseNativeHorizontalNavigation(event)
    ) {
      event.preventDefault();
      selectAdjacentCategory(event.key === "ArrowLeft" ? -1 : 1);

      return;
    }

    if (event.key !== "Tab") return;

    event.preventDefault();

    const nextGroupId = selectAdjacentCustomGroup(
      visibleCustomGroups,
      groupId,
      event.shiftKey,
    );

    if (!nextGroupId) return;

    toggleCustomGroup(nextGroupId);
  };

  useKeyboardEvent("keydown", handleKeyDown);

  const toggleRange = () => {
    clipboardViewState.range =
      clipboardViewState.range === "all" ? "favorite" : "all";
  };

  const selectAdjacentCategory = (direction: -1 | 1) => {
    const options = CATEGORY_GROUP_OPTIONS.map((option) => {
      return option.value;
    });
    const currentCategory = clipboardViewState.category;
    const current = currentCategory ? options.indexOf(currentCategory) : -1;
    const startIndex = direction === 1 ? -1 : options.length;
    const nextIndex =
      (current === -1 ? startIndex + direction : current + direction) %
      options.length;
    const normalizedIndex = (nextIndex + options.length) % options.length;

    clipboardViewState.range = "all";
    clipboardViewState.category = options[normalizedIndex];
  };

  const openCreateModal = () => {
    setModalMode("create");
    setEditingGroup(null);
    setModalOpen(true);
  };

  const openEditModal = (record: ClipboardGroupRecord) => {
    setModalMode("edit");
    setEditingGroup(record);
    setModalOpen(true);
  };

  const closeModal = () => {
    setModalOpen(false);
    setEditingGroup(null);
  };

  const handleModalSubmit = async (input: ClipboardGroupInput) => {
    if (modalMode === "create") {
      await createClipboardGroup(input);
      closeModal();
      return;
    }

    if (!editingGroup) return;

    await updateClipboardGroup(editingGroup.id, input);
    closeModal();
  };

  const handleGroupMenuClick = (info: { key: string }) => {
    const record = contextGroupRef.current;
    if (!record) return;

    const action = parseGroupMenuAction(info.key);
    if (!action) return;

    if (action === GROUP_MENU_ACTION.EDIT) {
      openEditModal(record);
      return;
    }

    if (action === GROUP_MENU_ACTION.HIDE) {
      void updateClipboardGroup(record.id, {
        icon: record.icon,
        isHidden: true,
        name: record.name,
      });
      return;
    }

    if (action === GROUP_MENU_ACTION.DELETE) {
      requestDeleteGroup(record);
    }
  };

  const handleCreateGroupAction = () => {
    openCreateModal();
  };

  const openGroupPreference = async () => {
    await openPreferenceWithHighlight(CUSTOM_GROUPS_SETTING_ID);
  };

  const handleMoreMenuClick = async (info: { key: string }) => {
    const action = parseMoreMenuAction(info.key);
    if (action === MORE_MENU_ACTION.NEW_GROUP) {
      handleCreateGroupAction();
      return;
    }

    if (action === MORE_MENU_ACTION.MANAGE_GROUPS) {
      await openGroupPreference();
      return;
    }

    const id = parseMoreMenuGroupId(info.key);
    if (!id) return;

    toggleCustomGroup(id);
  };

  const requestDeleteGroup = (record: ClipboardGroupRecord) => {
    deleteGroupRef.current = record;

    getModalApi().confirm({
      centered: true,
      content: (
        <span className="text-ant-secondary text-sm">
          {t("clipboard:groups.deleteConfirmDescription", {
            group: record.name,
          })}
        </span>
      ),
      okButtonProps: { danger: true },
      okText: t("common:actions.delete"),
      onOk: confirmDeleteGroup,
      title: t("clipboard:groups.delete"),
    });
  };

  const confirmDeleteGroup = async () => {
    const record = deleteGroupRef.current;
    if (!record) return;

    await deleteClipboardGroup(record.id);

    if (clipboardViewState.groupId === record.id) {
      clipboardViewState.groupId = null;
    }

    deleteGroupRef.current = null;
  };

  const groupMenuItems = buildGroupActionMenuItems(t);
  const createMenuItems = buildCreateMenuItems(t);

  const handleOverflowGroupContext = (record: ClipboardGroupRecord) => {
    contextGroupRef.current = record;
  };

  const moreMenuItems = buildMoreMenuItems(
    overflowCustomGroups,
    groupMenuItems,
    handleGroupMenuClick,
    handleOverflowGroupContext,
    t,
  );
  const moreMenuSelectedKeys = groupId ? [buildMoreMenuGroupKey(groupId)] : [];
  const moreButtonSelected = overflowCustomGroups.some((record) => {
    return record.id === groupId;
  });

  const renderMoreButton = () => {
    if (overflowCustomGroups.length === 0) return null;

    return (
      <Dropdown
        menu={{
          items: moreMenuItems,
          onClick: handleMoreMenuClick,
          selectedKeys: moreMenuSelectedKeys,
        }}
        tooltip={t("clipboard:groups.more")}
        trigger={["click"]}
      >
        <button
          className={cn(GROUP_BUTTON_BASE_CLASS, {
            "bg-ant-primary text-ant-light-solid": moreButtonSelected,
            "text-ant-secondary hover:bg-ant-fill-tertiary":
              !moreButtonSelected,
          })}
          type="button"
        >
          <KeyHint hintKey="N" onKeyPress={handleCreateGroupAction}>
            <i aria-hidden className="i-lucide:more-horizontal text-sm!" />
          </KeyHint>
        </button>
      </Dropdown>
    );
  };

  const renderCreateButton = () => {
    if (overflowCustomGroups.length > 0) return null;

    return (
      <Dropdown
        menu={{
          items: createMenuItems,
          onClick: handleMoreMenuClick,
        }}
        tooltip={t("clipboard:groups.add")}
        trigger={["contextMenu"]}
      >
        <button
          className={cn(
            GROUP_BUTTON_BASE_CLASS,
            "text-ant-secondary hover:bg-ant-fill-tertiary",
          )}
          onClick={handleCreateGroupAction}
          type="button"
        >
          <KeyHint hintKey="N" onKeyPress={handleCreateGroupAction}>
            <i aria-hidden className="i-lucide:plus text-sm!" />
          </KeyHint>
        </button>
      </Dropdown>
    );
  };

  const renderRangeButton = ({ labelKey, value, icon }: RangeGroupOption) => {
    const selected = range === value;
    const nextRange =
      range === "all" ? "favorite" : range === "favorite" ? "all" : void 0;
    const showShortcutHint = nextRange === value;

    return renderFilterButton({
      icon,
      label: t(`clipboard:${labelKey}`),
      selected,
      showShortcutHint,
      type: "range",
      value,
    });
  };

  const renderCategoryButton = ({
    labelKey,
    value,
    icon,
  }: CategoryGroupOption) => {
    const selected = category === value;

    return renderFilterButton({
      icon,
      label: t(`clipboard:${labelKey}`),
      selected,
      type: "category",
      value,
    });
  };

  const renderFilterButton = (options: {
    icon: ClipboardGroupIconValue;
    label: string;
    selected: boolean;
    showShortcutHint?: boolean;
    type: "category" | "range";
    value: ClipboardCategory | ClipboardRange;
  }) => {
    const { icon, label, selected, showShortcutHint, type, value } = options;

    return (
      <Tooltip key={`${type}:${value}`} title={label}>
        <button
          className={cn(
            GROUP_ICON_BUTTON_CLASS,
            selected
              ? resolveFilterSelectedClass(value)
              : "text-ant-secondary hover:bg-ant-fill-tertiary",
          )}
          data-type={type}
          data-value={value}
          onClick={handleGroupClick}
          type="button"
        >
          {showShortcutHint ? (
            <KeyHint hintKey="Q">
              <ClipboardGroupIcon icon={icon} selected={selected} />
            </KeyHint>
          ) : (
            <ClipboardGroupIcon icon={icon} selected={selected} />
          )}
        </button>
      </Tooltip>
    );
  };

  return (
    <>
      <div
        className="flex items-center gap-1 overflow-hidden bg-slate-100 px-3 pb-2 dark:bg-neutral-900"
        data-tauri-drag-region
        ref={toolbarRef}
      >
        {RANGE_GROUP_OPTIONS.map(renderRangeButton)}
        <GroupSeparator />
        {CATEGORY_GROUP_OPTIONS.map(renderCategoryButton)}
        <GroupSeparator separatorRef={customGroupAnchorRef} />

        {inlineCustomGroups.length > 0 && (
          <div className="flex min-w-0 shrink-0 items-center gap-1 overflow-hidden">
            {inlineCustomGroups.map((record) => {
              const selected = groupId === record.id;

              return (
                <Dropdown
                  key={record.id}
                  menu={{
                    items: groupMenuItems,
                    onClick: handleGroupMenuClick,
                  }}
                  tooltip={record.name}
                  trigger={["contextMenu"]}
                >
                  <button
                    className={cn(GROUP_ICON_BUTTON_CLASS, {
                      "bg-ant-primary text-ant-light-solid": selected,
                      "text-ant-secondary hover:bg-ant-fill-tertiary":
                        !selected,
                    })}
                    data-group-id={record.id}
                    onClick={handleGroupClick}
                    onContextMenu={handleCustomGroupContextMenu}
                    type="button"
                  >
                    <ClipboardGroupIcon
                      icon={record.icon}
                      selected={selected}
                    />
                  </button>
                </Dropdown>
              );
            })}
          </div>
        )}

        {renderMoreButton()}
        {renderCreateButton()}
      </div>

      <ClipboardGroupModal
        group={editingGroup}
        mode={modalMode}
        onCancel={closeModal}
        onSubmit={handleModalSubmit}
        open={modalOpen}
      />
    </>
  );
};

const GroupSeparator: FC<GroupSeparatorProps> = (props) => {
  const { separatorRef } = props;

  return (
    <span
      aria-hidden
      className="mx-1 h-4 w-px shrink-0 bg-ant-split"
      ref={separatorRef}
    />
  );
};

const OverflowGroupMenuLabel: FC<OverflowGroupMenuLabelProps> = (props) => {
  const { menuItems, onContext, onMenuClick, record } = props;

  const handleContextMenu = () => {
    onContext(record);
  };

  return (
    <Dropdown
      menu={{
        items: menuItems,
        onClick: onMenuClick,
      }}
      trigger={["contextMenu"]}
    >
      <span
        className="flex min-w-28 items-center gap-2"
        onContextMenu={handleContextMenu}
        role="menuitem"
        tabIndex={-1}
      >
        <ClipboardGroupIcon icon={record.icon} inheritColor />
        <span>{record.name}</span>
      </span>
    </Dropdown>
  );
};

/** Commit visible custom-group count after the next render. */
function scheduleVisibleCustomGroupCountUpdate(
  toolbarRef: RefObject<HTMLDivElement | null>,
  customAnchorRef: RefObject<HTMLSpanElement | null>,
  setVisibleCustomGroupCount: Dispatch<SetStateAction<number>>,
  groupCount: number,
) {
  requestAnimationFrame(() => {
    commitVisibleCustomGroupCount(
      toolbarRef.current,
      customAnchorRef.current,
      setVisibleCustomGroupCount,
      groupCount,
    );
  });
}

/** Write the number of custom groups that fit in the toolbar. */
function commitVisibleCustomGroupCount(
  toolbar: HTMLDivElement | null,
  customAnchor: HTMLSpanElement | null,
  setVisibleCustomGroupCount: Dispatch<SetStateAction<number>>,
  groupCount: number,
) {
  const rawCapacity =
    toolbar && customAnchor
      ? computeCustomGroupCapacity(toolbar, customAnchor)
      : groupCount;
  const visibleCount = Math.min(groupCount, rawCapacity);

  setVisibleCustomGroupCount((current) => {
    if (current === visibleCount) return current;

    return visibleCount;
  });
}

/** Compute how many custom groups fit in the remaining toolbar width. */
function computeCustomGroupCapacity(
  toolbar: HTMLDivElement,
  customAnchor: HTMLSpanElement,
) {
  const toolbarRect = toolbar.getBoundingClientRect();
  const customRect = customAnchor.getBoundingClientRect();
  const customStart =
    customRect.right -
    toolbarRect.left +
    GROUP_SEPARATOR_MARGIN +
    GROUP_BUTTON_GAP;
  const actionSlotWidth = GROUP_BUTTON_GAP + GROUP_BUTTON_WIDTH;
  const availableWidth = Math.max(
    0,
    toolbar.clientWidth - customStart - actionSlotWidth,
  );

  return Math.max(
    0,
    Math.floor(
      (availableWidth + GROUP_BUTTON_GAP) /
        (GROUP_BUTTON_WIDTH + GROUP_BUTTON_GAP),
    ),
  );
}

/** Build the shared context menu for inline and overflow groups. */
function buildGroupActionMenuItems(
  t: TFunction<["clipboard", "common"]>,
): DropdownMenuItems {
  return [
    {
      icon: "i-lucide:pencil",
      key: GROUP_MENU_ACTION.EDIT,
      label: t("clipboard:groups.edit"),
    },
    {
      icon: "i-lucide:eye-off",
      key: GROUP_MENU_ACTION.HIDE,
      label: t("clipboard:groups.hide"),
    },
    { type: "divider" },
    {
      danger: true,
      icon: "i-lucide:trash-2",
      key: GROUP_MENU_ACTION.DELETE,
      label: t("clipboard:groups.delete"),
    },
  ];
}

/** Build the add button menu, retaining add on the left click. */
function buildCreateMenuItems(
  t: TFunction<["clipboard", "common"]>,
): DropdownMenuItems {
  return [
    {
      icon: "i-lucide:settings-2",
      key: MORE_MENU_ACTION.MANAGE_GROUPS,
      label: t("clipboard:groups.manage"),
    },
  ];
}

/** Build the more menu with management and overflow shortcuts. */
function buildMoreMenuItems(
  groups: ClipboardGroupRecord[],
  groupMenuItems: DropdownMenuItems,
  onGroupMenuClick: (info: { key: string }) => void,
  onGroupContext: (record: ClipboardGroupRecord) => void,
  t: TFunction<["clipboard", "common"]>,
): DropdownMenuItems {
  const groupItems = groups.map((record) => {
    return {
      key: buildMoreMenuGroupKey(record.id),
      label: (
        <OverflowGroupMenuLabel
          menuItems={groupMenuItems}
          onContext={onGroupContext}
          onMenuClick={onGroupMenuClick}
          record={record}
        />
      ),
    };
  });

  if (groupItems.length === 0) {
    return [
      {
        icon: "i-lucide:plus",
        key: MORE_MENU_ACTION.NEW_GROUP,
        label: t("clipboard:groups.add"),
      },
      {
        icon: "i-lucide:settings-2",
        key: MORE_MENU_ACTION.MANAGE_GROUPS,
        label: t("clipboard:groups.manage"),
      },
    ];
  }

  return [
    ...groupItems,
    { type: "divider" },
    {
      icon: "i-lucide:plus",
      key: MORE_MENU_ACTION.NEW_GROUP,
      label: t("clipboard:groups.add"),
    },
    {
      icon: "i-lucide:settings-2",
      key: MORE_MENU_ACTION.MANAGE_GROUPS,
      label: t("clipboard:groups.manage"),
    },
  ];
}

function parseGroupMenuAction(key: string): GroupMenuAction | null {
  const actions = Object.values(GROUP_MENU_ACTION);
  if (!actions.includes(key as GroupMenuAction)) return null;

  return key as GroupMenuAction;
}

function parseMoreMenuAction(key: string): MoreMenuAction | null {
  const actions = Object.values(MORE_MENU_ACTION);
  if (!actions.includes(key as MoreMenuAction)) return null;

  return key as MoreMenuAction;
}

function buildMoreMenuGroupKey(id: string): MoreMenuGroupKey {
  return `group:${id}`;
}

function parseMoreMenuGroupId(key: string) {
  if (!key.startsWith("group:")) return null;

  return key.slice("group:".length);
}

function isRangeGroup(value: unknown): value is ClipboardRange {
  return RANGE_GROUP_OPTIONS.some((option) => {
    return option.value === value;
  });
}

function isCategoryGroup(value: unknown): value is ClipboardCategory {
  return CATEGORY_GROUP_OPTIONS.some((option) => {
    return option.value === value;
  });
}

function selectAdjacentCustomGroup(
  groups: ClipboardGroupRecord[],
  groupId: string | null,
  reverse: boolean,
) {
  if (groups.length === 0) return null;

  const currentIndex = groupId
    ? groups.findIndex((record) => {
        return record.id === groupId;
      })
    : -1;

  if (reverse) {
    if (currentIndex === -1) return groups[groups.length - 1]?.id ?? null;

    return (
      groups[(currentIndex - 1 + groups.length) % groups.length]?.id ?? null
    );
  }

  if (currentIndex === -1) return groups[0]?.id ?? null;

  return groups[(currentIndex + 1) % groups.length]?.id ?? null;
}

function shouldUseNativeHorizontalNavigation(event: KeyboardEvent) {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return false;

  const tagName = target.tagName.toLowerCase();
  if (target.isContentEditable) return true;

  return tagName === "input" || tagName === "textarea";
}

function ensureSelectedGroupStillExists(groups: ClipboardGroupRecord[]) {
  const selectedGroupId = clipboardViewState.groupId;
  if (!selectedGroupId) return;

  const exists = groups.some((record) => {
    return record.id === selectedGroupId;
  });
  if (exists) return;

  clipboardViewState.groupId = null;
}

export default Group;

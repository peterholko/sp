import greenstatus from "ui_comp/greenstatus.png";
import yellowstatus from "ui_comp/yellowstatus.png";
import redstatus from "ui_comp/redstatus.png";
import {
  DEHYDRATED,
  DEPLETED,
  ENERGIZED,
  EXHAUSTED,
  FAMISHED,
  HUNGRY,
  HYDRATED,
  NOURISHED,
  PARCHED,
  PECKISH,
  RAVENOUS,
  REFRESHED,
  RESTORED,
  SATIATED,
  SLIGHTLY_THIRSTY,
  THIRSTY,
  TIRED,
  WEARY,
} from "../config";

export type NeedKind = "thirst" | "hunger" | "tiredness";
export type NeedSeverity = "good" | "warning" | "danger";

const NEED_SEVERITY: Record<NeedKind, Record<string, NeedSeverity>> = {
  thirst: {
    [HYDRATED]: "good",
    [REFRESHED]: "good",
    [SLIGHTLY_THIRSTY]: "warning",
    [THIRSTY]: "warning",
    [PARCHED]: "danger",
    [DEHYDRATED]: "danger",
  },
  hunger: {
    [SATIATED]: "good",
    [NOURISHED]: "good",
    [HUNGRY]: "warning",
    [PECKISH]: "warning",
    [FAMISHED]: "danger",
    [RAVENOUS]: "danger",
  },
  tiredness: {
    [ENERGIZED]: "good",
    [RESTORED]: "good",
    [WEARY]: "warning",
    [TIRED]: "warning",
    [EXHAUSTED]: "danger",
    [DEPLETED]: "danger",
  },
};

const NEED_STATUS_ICONS: Record<NeedSeverity, string> = {
  good: greenstatus,
  warning: yellowstatus,
  danger: redstatus,
};

export function getNeedSeverity(kind: NeedKind, value?: string): NeedSeverity | undefined {
  if (!value) {
    return undefined;
  }

  return NEED_SEVERITY[kind][value];
}

export function getNeedStatusIcon(kind: NeedKind, value?: string): string | undefined {
  const severity = getNeedSeverity(kind, value);
  return severity ? NEED_STATUS_ICONS[severity] : undefined;
}

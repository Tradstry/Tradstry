import {
  Analytics01Icon,
  BitcoinIcon,
  ChartIncreaseIcon,
  ChartLineData01Icon,
  FlashIcon,
  Globe02Icon,
  MoneyBag02Icon,
  PieChartIcon,
  Shield01Icon,
  Target02Icon,
} from "@hugeicons/core-free-icons";
import type { IconSvgElement } from "@hugeicons/react";

export const ACCOUNT_ICONS: Record<string, IconSvgElement> = {
  "chart-line-data-01": ChartLineData01Icon,
  "pie-chart": PieChartIcon,
  "analytics-01": Analytics01Icon,
  "money-bag-02": MoneyBag02Icon,
  bitcoin: BitcoinIcon,
  "globe-02": Globe02Icon,
  "target-02": Target02Icon,
  flash: FlashIcon,
  "shield-01": Shield01Icon,
  "chart-increase": ChartIncreaseIcon,
};

export const DEFAULT_ICON = "chart-line-data-01";

export const ICON_OPTIONS = Object.keys(ACCOUNT_ICONS);

export type DashboardRouteMeta = {
  title: string;
  description: string;
  section: string;
  showPageHeader?: boolean;
};

export const dashboardRouteMeta: Record<string, DashboardRouteMeta> = {
  "/dashboard": {
    title: "Dashboard",
    description:
      "Your trading home base for portfolio activity, recent journal entries, and account-aware shortcuts.",
    section: "Home",
    showPageHeader: false,
  },
  "/dashboard/journal": {
    title: "Journal",
    description:
      "Capture trades, reasoning, and post-trade notes in one route that now exists in the app router.",
    section: "Home",
    showPageHeader: false,
  },
  "/dashboard/notebook": {
    title: "Notebook",
    description:
      "Centralize market notes, screenshots, and structured research inside a dedicated notebook route.",
    section: "Home",
    showPageHeader: false,
  },
  "/dashboard/playbook": {
    title: "Playbook",
    description: "",
    section: "",
    showPageHeader: false,
  },
  "/dashboard/statistics": {
    title: "Statistics",
    description:
      "Review win rate, expectancy, and distribution metrics inside the statistics workspace.",
    section: "Analytics",
  },
  "/dashboard/reporting": {
    title: "Reporting",
    description:
      "Prepare shareable summaries and performance reporting on a dedicated reporting route.",
    section: "Analytics",
  },
  "/dashboard/ai-reports": {
    title: "AI Reports",
    description:
      "Generate AI-assisted summaries and route them through a proper page instead of a placeholder link.",
    section: "AI Stuff",
  },
  "/dashboard/ai-insights": {
    title: "AI Insights",
    description:
      "Surface model-generated observations, patterns, and anomalies in the AI insights route.",
    section: "AI Stuff",
  },
  "/dashboard/mindset-lab": {
    title: "MindsetLab",
    description:
      "Track discipline, routines, and trading psychology in the MindsetLab route.",
    section: "Resources",
  },
  "/dashboard/markets": {
    title: "Markets",
    description:
      "Use this route for market overviews, watchlists, and higher-level market context.",
    section: "Resources",
  },
  "/dashboard/charting": {
    title: "Charting",
    description:
      "Build chart review tools and annotations inside the charting workspace.",
    section: "Resources",
  },
};

export function getDashboardRouteMeta(pathname: string): DashboardRouteMeta {
  return (
    dashboardRouteMeta[pathname] ?? {
      title: "Dashboard",
      description: "Shared workspace for the active dashboard route.",
      section: "Tradstry",
    }
  );
}

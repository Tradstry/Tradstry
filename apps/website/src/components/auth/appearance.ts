import type { ClerkProvider } from "@clerk/nextjs";
import type * as React from "react";

type Appearance = NonNullable<
  React.ComponentProps<typeof ClerkProvider>["appearance"]
>;

/** Clerk computes hover/focus shades from these, so they must be real colors — not CSS vars. */
export const inkAppearance: Appearance = {
  options: {
    logoPlacement: "none",
    socialButtonsPlacement: "top",
    socialButtonsVariant: "blockButton",
    showOptionalFields: false,
    termsPageUrl: "/terms",
    privacyPageUrl: "/privacy",
  },
  variables: {
    colorPrimary: "#FAFAFA",
    colorPrimaryForeground: "#0A0A0B",
    colorBackground: "#131316",
    colorForeground: "#FAFAFA",
    colorMutedForeground: "#8B8B93",
    colorMuted: "#0E0E11",
    colorInput: "#0E0E11",
    colorInputForeground: "#FAFAFA",
    colorBorder: "#FFFFFF1A",
    colorRing: "#FFFFFF80",
    colorShadow: "#000000",
    colorModalBackdrop: "#0A0A0BCC",
    colorDanger: "#F87171",
    colorSuccess: "#34D399",
    colorNeutral: "#FFFFFF",
    borderRadius: "0.625rem",
    fontFamily: "var(--font-sans)",
    fontFamilyMono: "var(--font-geist-mono)",
    fontSize: "0.9375rem",
  },
  elements: {
    rootBox: "w-full",
    cardBox: "w-full shadow-2xl shadow-black/60",
    card: "border border-white/10 bg-[#131316] px-7 py-8",
    header: "hidden",

    socialButtonsBlockButton:
      "h-10 border border-white/10 bg-white/[0.03] text-zinc-200 transition-colors hover:bg-white/[0.07]",
    socialButtonsBlockButtonText: "font-normal",

    dividerLine: "bg-white/10",
    dividerText: "text-xs uppercase tracking-[0.14em] text-zinc-500",

    formFieldLabel: "text-xs text-zinc-400",
    formFieldInput:
      "h-10 border border-white/10 bg-[#0E0E11] text-zinc-50 placeholder:text-zinc-600",
    formFieldInputShowPasswordButton: "text-zinc-500 hover:text-zinc-200",
    formFieldAction: "text-xs text-zinc-400 hover:text-zinc-50",
    formFieldHintText: "text-xs text-zinc-500",
    formFieldErrorText: "text-xs text-[#F87171]",

    formButtonPrimary:
      "h-10 bg-zinc-50 text-[15px] font-medium normal-case text-[#0A0A0B] shadow-none transition-colors hover:bg-zinc-200",
    formButtonReset: "text-zinc-400 hover:text-zinc-50",

    otpCodeFieldInput:
      "border border-white/10 bg-[#0E0E11] font-mono text-zinc-50",
    formResendCodeLink: "text-zinc-400 hover:text-zinc-50",

    identityPreview: "border border-white/10 bg-white/[0.03]",
    identityPreviewText: "text-zinc-300",
    identityPreviewEditButton: "text-zinc-400 hover:text-zinc-50",

    footer: "bg-transparent",
    footerAction: "bg-transparent",
    footerActionText: "text-zinc-500",
    footerActionLink:
      "font-medium text-zinc-200 underline-offset-4 hover:text-zinc-50",

    alert: "border border-white/10 bg-white/[0.03] text-zinc-300",
    alertText: "text-zinc-300",
  },
};

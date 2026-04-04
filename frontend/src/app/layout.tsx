import { ClerkProvider } from "@clerk/nextjs";
import { ThemeProvider } from "next-themes";
import { TooltipProvider } from "@/components/ui/tooltip"
import type { Metadata } from "next";
import { Geist, Geist_Mono, Inter } from "next/font/google";
import "./globals.css";
import { cn } from "@/lib/utils";
import { Toaster } from "@/components/ui/sonner";

const inter = Inter({subsets:['latin'],variable:'--font-sans'});

const geistSans = Geist({
	variable: "--font-geist-sans",
	subsets: ["latin"],
});

const geistMono = Geist_Mono({
	variable: "--font-geist-mono",
	subsets: ["latin"],
});

export const metadata: Metadata = {
	title: "Tradstry",
	description: "Trading journal and analytics",
};

export default function RootLayout({
	children,
}: Readonly<{
	children: React.ReactNode;
}>) {
	return (
		<ClerkProvider>
			<html lang="en" className={cn("font-sans", inter.variable)} suppressHydrationWarning>
				<body
					className={`${geistSans.variable} ${geistMono.variable} antialiased overflow-x-hidden`}
				>
					<ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
						<TooltipProvider>
							{children}
							<Toaster />
						</TooltipProvider>
					</ThemeProvider>
				</body>
			</html>
		</ClerkProvider>
	);
}

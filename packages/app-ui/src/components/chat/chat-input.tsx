"use client";

import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
	type ChangeEvent,
	type KeyboardEvent,
	useEffect,
	useRef,
	useState,
} from "react";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import {
	useDeleteUserAgent,
	useUserAgents,
} from "@tradstry/app-ui/hooks/agents";
import { useChatStore, useSendMessage } from "@tradstry/app-ui/hooks/chat";
import { useJournalEntriesForWorkspace } from "@tradstry/app-ui/hooks/journal";
import { usePlaybooks } from "@tradstry/app-ui/hooks/playbook";
import {
	useCreateUserPrompt,
	useDeleteUserPrompt,
	useUpdateUserPrompt,
	useUserPrompts,
} from "@tradstry/app-ui/hooks/prompts";
import type { UserPrompt } from "@tradstry/app-ui/lib/service/prompts";
import { ChatContextPicker } from "./chat-context-picker";

interface ChatInputProps {
	sessionId: string;
	workspaceId: string;
}

const SLASH_COMMANDS = [
	{
		command: "/report",
		label: "Generate Report",
		description: "Full performance report for a date range",
		prompt:
			"Generate a full trading performance report including total P&L, win rate, average R, profit factor, streaks, and per-symbol breakdown.",
	},
	{
		command: "/analysis",
		label: "Generate Analysis",
		description: "Deep analysis of patterns and insights",
		prompt:
			"Analyze my recent trading patterns. Look for recurring setups, common mistakes, and actionable insights to improve my performance.",
	},
	{
		command: "/research",
		label: "Research Trades",
		description: "Comprehensive research on a symbol or topic",
		prompt:
			"Research my recent trades. Fetch the data, compute metrics, search for patterns, and give me a thorough analysis.",
	},
	{
		command: "/create-agent",
		label: "Create Agent",
		description: "Build a custom reusable AI workflow",
		prompt: "Create an agent for me. I want to build a custom workflow.",
	},
];

type SlashTab = "prompts" | "agents" | "saved";

export function ChatInput({ sessionId, workspaceId }: ChatInputProps) {
	const [text, setText] = useState("");
	const [pickerOpen, setPickerOpen] = useState(false);
	const [slashOpen, setSlashOpen] = useState(false);
	const [slashTab, setSlashTab] = useState<SlashTab>("prompts");
	const [slashIndex, setSlashIndex] = useState(0);
	const { isStreaming, pinnedContext, setPinnedContext, resetStream } =
		useChatStore();
	const sendMessage = useSendMessage(workspaceId);
	const textareaRef = useRef<HTMLTextAreaElement>(null);

	const workspace = useActiveWorkspace();
	const { data: agents = [] } = useUserAgents(workspace?.id ?? null);
	const { data: trades = [] } = useJournalEntriesForWorkspace(
		workspace?.id ?? null,
	);
	const { data: playbooks = [] } = usePlaybooks();
	const deleteAgent = useDeleteUserAgent();

	const { data: savedPrompts = [] } = useUserPrompts();
	const createPrompt = useCreateUserPrompt();
	const updatePrompt = useUpdateUserPrompt();
	const deletePromptMutation = useDeleteUserPrompt();
	const [promptForm, setPromptForm] = useState<{
		mode: "create" | "edit";
		id?: string;
		name: string;
		content: string;
	} | null>(null);

	// Clear input when switching sessions
	useEffect(() => {
		setText("");
		setPickerOpen(false);
		const marketSymbol = useChatStore.getState().pinnedContext.marketSymbol;
		setPinnedContext(marketSymbol ? { marketSymbol } : {});
	}, [sessionId, setPinnedContext]);

	// Auto-grow the textarea with its content, up to the CSS max-height (then it scrolls).
	useEffect(() => {
		const ta = textareaRef.current;
		if (!ta) return;
		ta.style.height = "auto";
		ta.style.height = `${ta.scrollHeight}px`;
	}, [text]);

	const hasPinnedContext =
		!!pinnedContext.dateRange ||
		(pinnedContext.tradeIds?.length ?? 0) > 0 ||
		(pinnedContext.playbookIds?.length ?? 0) > 0 ||
		Boolean(pinnedContext.marketSymbol);

	function closePicker() {
		setPickerOpen(false);
		requestAnimationFrame(() => textareaRef.current?.focus());
	}

	function removeTradeContext(tradeId: string) {
		setPinnedContext({
			...pinnedContext,
			tradeIds: pinnedContext.tradeIds?.filter((id) => id !== tradeId),
		});
	}

	function removePlaybookContext(playbookId: string) {
		setPinnedContext({
			...pinnedContext,
			playbookIds: pinnedContext.playbookIds?.filter((id) => id !== playbookId),
		});
	}

	function removeDateRangeContext() {
		setPinnedContext({ ...pinnedContext, dateRange: undefined });
	}

	function removeMarketContext() {
		setPinnedContext({ ...pinnedContext, marketSymbol: undefined });
	}

	// Filter commands based on what user typed after /
	const query = slashOpen ? text.slice(1).toLowerCase() : "";
	const filteredCommands = SLASH_COMMANDS.filter(
		(cmd) =>
			cmd.command.toLowerCase().includes(query) ||
			cmd.label.toLowerCase().includes(query),
	);
	const filteredAgents = agents.filter((a) =>
		a.name.toLowerCase().includes(query),
	);
	const filteredSaved = savedPrompts.filter(
		(p) =>
			p.name.toLowerCase().includes(query) ||
			p.content.toLowerCase().includes(query),
	);

	// Current list based on active tab
	const currentList =
		slashTab === "prompts"
			? filteredCommands
			: slashTab === "agents"
				? filteredAgents
				: filteredSaved;

	// Close slash menu if text no longer starts with /
	useEffect(() => {
		if (slashOpen && !text.startsWith("/")) {
			setSlashOpen(false);
		}
	}, [text, slashOpen]);

	// Clamp index when list changes
	useEffect(() => {
		if (slashIndex >= currentList.length) {
			setSlashIndex(Math.max(0, currentList.length - 1));
		}
	}, [currentList.length, slashIndex]);

	function handleSend() {
		const content = text.trim();
		if (!content) return;
		resetStream();
		sendMessage.mutate({
			sessionId,
			content,
			context: hasPinnedContext ? pinnedContext : undefined,
		});
		setText("");
		textareaRef.current?.focus();
	}

	function selectSlashCommand(cmd: (typeof SLASH_COMMANDS)[number]) {
		setSlashOpen(false);
		setText("");
		resetStream();
		sendMessage.mutate({
			sessionId,
			content: cmd.prompt,
			context: hasPinnedContext ? pinnedContext : undefined,
		});
		textareaRef.current?.focus();
	}

	function selectAgent(agent: { name: string }) {
		setSlashOpen(false);
		setText("");
		resetStream();
		sendMessage.mutate({
			sessionId,
			content: `Run my ${agent.name}`,
			context: hasPinnedContext ? pinnedContext : undefined,
		});
		textareaRef.current?.focus();
	}

	function selectSavedPrompt(prompt: UserPrompt) {
		setSlashOpen(false);
		setText("");
		resetStream();
		sendMessage.mutate({
			sessionId,
			content: prompt.content,
			context: hasPinnedContext ? pinnedContext : undefined,
		});
		textareaRef.current?.focus();
	}

	function handlePromptFormSubmit() {
		if (!promptForm || !promptForm.name.trim() || !promptForm.content.trim())
			return;
		if (promptForm.mode === "create") {
			createPrompt.mutate({
				name: promptForm.name.trim(),
				content: promptForm.content.trim(),
			});
		} else if (promptForm.id) {
			updatePrompt.mutate({
				id: promptForm.id,
				name: promptForm.name.trim(),
				content: promptForm.content.trim(),
			});
		}
		setPromptForm(null);
	}

	function sendPrompt(content: string) {
		setSlashOpen(false);
		setText("");
		resetStream();
		sendMessage.mutate({
			sessionId,
			content,
			context: hasPinnedContext ? pinnedContext : undefined,
		});
		textareaRef.current?.focus();
	}

	function handleChange(e: ChangeEvent<HTMLTextAreaElement>) {
		const val = e.target.value;
		const caret = e.target.selectionStart ?? val.length;
		const textBeforeCaret = val.slice(0, caret);

		// Treat an @ typed at the start of a word as the context-picker shortcut.
		// This leaves email addresses and @ symbols inside words alone.
		if (!isStreaming && /(^|\s)@$/.test(textBeforeCaret)) {
			setText(`${val.slice(0, caret - 1)}${val.slice(caret)}`);
			setSlashOpen(false);
			setPickerOpen(true);
			return;
		}

		setText(val);

		if (val === "/") {
			setPickerOpen(false);
			setSlashOpen(true);
			setSlashIndex(0);
			setSlashTab("prompts");
		}
	}

	function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
		if (slashOpen && currentList.length > 0) {
			if (e.key === "ArrowDown") {
				e.preventDefault();
				setSlashIndex((i) => (i + 1) % currentList.length);
				return;
			}
			if (e.key === "ArrowUp") {
				e.preventDefault();
				setSlashIndex((i) => (i - 1 + currentList.length) % currentList.length);
				return;
			}
			if (e.key === "Tab") {
				e.preventDefault();
				setSlashTab((t) =>
					t === "prompts" ? "agents" : t === "agents" ? "saved" : "prompts",
				);
				setSlashIndex(0);
				return;
			}
			if (e.key === "Enter") {
				e.preventDefault();
				if (slashTab === "prompts" && filteredCommands[slashIndex]) {
					selectSlashCommand(filteredCommands[slashIndex]);
				} else if (slashTab === "agents" && filteredAgents[slashIndex]) {
					selectAgent(filteredAgents[slashIndex]);
				} else if (slashTab === "saved" && filteredSaved[slashIndex]) {
					selectSavedPrompt(filteredSaved[slashIndex]);
				}
				return;
			}
			if (e.key === "Escape") {
				e.preventDefault();
				setSlashOpen(false);
				return;
			}
		}

		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	return (
		<div className="relative px-3 py-3">
			<div className="rounded-2xl border border-border bg-background p-3 shadow-sm">
				{/* Pinned context chips */}
				{hasPinnedContext && (
					<div className="mb-2 flex flex-wrap gap-1">
						{pinnedContext.tradeIds?.map((tradeId) => {
							const trade = trades.find((item) => item.id === tradeId);
							return (
								<span
									key={tradeId}
									className="inline-flex items-center gap-1 rounded-full border border-border bg-muted px-2 py-0.5 text-xs text-foreground"
								>
									@{trade?.symbol ?? "Trade"}
									<button
										type="button"
										onClick={() => removeTradeContext(tradeId)}
										className="ml-0.5 text-muted-foreground hover:text-foreground"
										aria-label={`Remove ${trade?.symbol ?? "trade"} context`}
									>
										<HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
									</button>
								</span>
							);
						})}
						{pinnedContext.playbookIds?.map((playbookId) => {
							const playbook = playbooks.find((item) => item.id === playbookId);
							return (
								<span
									key={playbookId}
									className="inline-flex items-center gap-1 rounded-full border border-border bg-muted px-2 py-0.5 text-xs text-foreground"
								>
									@{playbook?.name ?? "Playbook"}
									<button
										type="button"
										onClick={() => removePlaybookContext(playbookId)}
										className="ml-0.5 text-muted-foreground hover:text-foreground"
										aria-label={`Remove ${playbook?.name ?? "playbook"} context`}
									>
										<HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
									</button>
								</span>
							);
						})}
						{pinnedContext.dateRange && (
							<span className="inline-flex items-center gap-1 rounded-full border border-border bg-muted px-2 py-0.5 text-xs text-foreground">
								{pinnedContext.dateRange.from} – {pinnedContext.dateRange.to}
								<button
									type="button"
									onClick={removeDateRangeContext}
									className="ml-0.5 text-muted-foreground hover:text-foreground"
									aria-label="Remove date range"
								>
									<HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
								</button>
							</span>
						)}
						{pinnedContext.marketSymbol && (
							<span className="inline-flex items-center gap-1 rounded-full border border-blue-500/30 bg-blue-500/10 px-2 py-0.5 text-xs text-foreground">
								@{pinnedContext.marketSymbol} research
								<button
									type="button"
									onClick={removeMarketContext}
									className="ml-0.5 text-muted-foreground hover:text-foreground"
									aria-label="Remove market symbol context"
								>
									<HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
								</button>
							</span>
						)}
					</div>
				)}

				{/* @ Add context button */}
				<div className="mb-2">
					<button
						type="button"
						onClick={() => {
							setSlashOpen(false);
							setPickerOpen((open) => !open);
						}}
						disabled={isStreaming}
						className="inline-flex items-center gap-1.5 rounded-full border border-border px-3 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
					>
						<span className="font-medium">@</span>
						Add context
					</button>
					{pickerOpen && (
						<ChatContextPicker
							workspaceId={workspaceId}
							onClose={closePicker}
						/>
					)}
				</div>

				{/* Slash command popover with tabs */}
				{slashOpen && (
					<div className="absolute bottom-full left-3 right-3 z-50 mb-2 overflow-hidden rounded-xl border border-border bg-popover shadow-md">
						{/* Tab bar */}
						<div className="flex border-b border-border">
							<button
								onMouseDown={(e) => {
									e.preventDefault();
									setSlashTab("prompts");
									setSlashIndex(0);
								}}
								className={`flex-1 px-3 py-2 text-xs font-medium transition-colors ${
									slashTab === "prompts"
										? "border-b-2 border-foreground text-foreground"
										: "text-muted-foreground hover:text-foreground"
								}`}
							>
								Prompts
							</button>
							<button
								onMouseDown={(e) => {
									e.preventDefault();
									setSlashTab("agents");
									setSlashIndex(0);
								}}
								className={`flex-1 px-3 py-2 text-xs font-medium transition-colors ${
									slashTab === "agents"
										? "border-b-2 border-foreground text-foreground"
										: "text-muted-foreground hover:text-foreground"
								}`}
							>
								Agents{" "}
								{agents.length > 0 && (
									<span className="ml-1 rounded-full bg-muted px-1.5 text-[0.6rem]">
										{agents.length}
									</span>
								)}
							</button>
							<button
								onMouseDown={(e) => {
									e.preventDefault();
									setSlashTab("saved");
									setSlashIndex(0);
								}}
								className={`flex-1 px-3 py-2 text-xs font-medium transition-colors ${
									slashTab === "saved"
										? "border-b-2 border-foreground text-foreground"
										: "text-muted-foreground hover:text-foreground"
								}`}
							>
								Saved{" "}
								{savedPrompts.length > 0 && (
									<span className="ml-1 rounded-full bg-muted px-1.5 text-[0.6rem]">
										{savedPrompts.length}
									</span>
								)}
							</button>
						</div>

						{/* Tab content */}
						<div className="max-h-64 overflow-y-auto">
							{slashTab === "prompts" &&
								(filteredCommands.length === 0 ? (
									<div className="px-3 py-4 text-center text-xs text-muted-foreground">
										No matching prompts
									</div>
								) : (
									filteredCommands.map((cmd, i) => (
										<button
											key={cmd.command}
											onMouseDown={(e) => {
												e.preventDefault();
												selectSlashCommand(cmd);
											}}
											className={`flex w-full flex-col gap-0.5 px-3 py-2.5 text-left transition-colors ${
												i === slashIndex ? "bg-muted" : "hover:bg-muted/50"
											}`}
										>
											<span className="text-sm font-medium text-foreground">
												{cmd.label}
											</span>
											<span className="text-xs text-muted-foreground">
												{cmd.description}
											</span>
										</button>
									))
								))}

							{slashTab === "agents" &&
								(filteredAgents.length === 0 ? (
									<div className="flex flex-col items-center gap-2 px-3 py-4">
										<span className="text-xs text-muted-foreground">
											{agents.length === 0
												? "No agents yet"
												: "No matching agents"}
										</span>
										<button
											onMouseDown={(e) => {
												e.preventDefault();
												sendPrompt(
													"Create an agent for me. I want to build a custom workflow.",
												);
											}}
											className="rounded-full border border-border px-3 py-1 text-xs text-foreground hover:bg-muted"
										>
											+ Create Agent
										</button>
									</div>
								) : (
									<>
										{filteredAgents.map((agent, i) => (
											<div
												key={agent.id}
												className={`flex items-center gap-2 px-3 py-2.5 transition-colors ${
													i === slashIndex ? "bg-muted" : "hover:bg-muted/50"
												}`}
											>
												{/* Click name to run */}
												<button
													onMouseDown={(e) => {
														e.preventDefault();
														selectAgent(agent);
													}}
													className="flex flex-1 flex-col gap-0.5 text-left"
												>
													<span className="text-sm font-medium text-foreground">
														{agent.name}
													</span>
													<span className="line-clamp-1 text-xs text-muted-foreground">
														{agent.goal}
													</span>
												</button>

												{/* Action icons */}
												<div className="flex shrink-0 items-center gap-1">
													<button
														onMouseDown={(e) => {
															e.preventDefault();
															sendPrompt(`Edit my ${agent.name}`);
														}}
														className="rounded p-1 text-muted-foreground hover:bg-background hover:text-foreground"
														title="Edit"
													>
														<svg
															width="12"
															height="12"
															viewBox="0 0 16 16"
															fill="none"
														>
															<path
																d="M11.5 1.5l3 3L5 14H2v-3L11.5 1.5z"
																stroke="currentColor"
																strokeWidth="1.5"
																strokeLinecap="round"
																strokeLinejoin="round"
															/>
														</svg>
													</button>
													<button
														onMouseDown={(e) => {
															e.preventDefault();
															if (confirm(`Delete agent "${agent.name}"?`)) {
																deleteAgent.mutate(agent.id);
															}
														}}
														className="rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
														title="Delete"
													>
														<svg
															width="12"
															height="12"
															viewBox="0 0 16 16"
															fill="none"
														>
															<path
																d="M2 4h12M5 4V2h6v2M6 7v5M10 7v5M3 4l1 10h8l1-10"
																stroke="currentColor"
																strokeWidth="1.5"
																strokeLinecap="round"
																strokeLinejoin="round"
															/>
														</svg>
													</button>
												</div>
											</div>
										))}
										<div className="border-t border-border px-3 py-2">
											<button
												onMouseDown={(e) => {
													e.preventDefault();
													sendPrompt(
														"Create an agent for me. I want to build a custom workflow.",
													);
												}}
												className="w-full rounded-lg border border-dashed border-border py-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
											>
												+ Create New Agent
											</button>
										</div>
									</>
								))}

							{slashTab === "saved" &&
								(promptForm ? (
									<div className="p-3 space-y-2">
										<input
											type="text"
											value={promptForm.name}
											onChange={(e) =>
												setPromptForm({ ...promptForm, name: e.target.value })
											}
											placeholder="Prompt name"
											className="w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
											autoFocus
											onMouseDown={(e) => e.stopPropagation()}
										/>
										<textarea
											value={promptForm.content}
											onChange={(e) =>
												setPromptForm({
													...promptForm,
													content: e.target.value,
												})
											}
											placeholder="Prompt content (what the AI will execute)"
											rows={3}
											className="w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring resize-none"
											onMouseDown={(e) => e.stopPropagation()}
										/>
										<div className="flex gap-2">
											<button
												onMouseDown={(e) => {
													e.preventDefault();
													handlePromptFormSubmit();
												}}
												disabled={
													!promptForm.name.trim() || !promptForm.content.trim()
												}
												className="rounded-md bg-foreground px-3 py-1 text-xs font-medium text-background hover:opacity-80 disabled:opacity-30"
											>
												{promptForm.mode === "create" ? "Save" : "Update"}
											</button>
											<button
												onMouseDown={(e) => {
													e.preventDefault();
													setPromptForm(null);
												}}
												className="rounded-md px-3 py-1 text-xs text-muted-foreground hover:text-foreground"
											>
												Cancel
											</button>
										</div>
									</div>
								) : filteredSaved.length === 0 ? (
									<div className="flex flex-col items-center gap-2 px-3 py-4">
										<span className="text-xs text-muted-foreground">
											{savedPrompts.length === 0
												? "No saved prompts yet"
												: "No matching prompts"}
										</span>
										<button
											onMouseDown={(e) => {
												e.preventDefault();
												setPromptForm({
													mode: "create",
													name: "",
													content: "",
												});
											}}
											className="rounded-full border border-border px-3 py-1 text-xs text-foreground hover:bg-muted"
										>
											+ Create Prompt
										</button>
									</div>
								) : (
									<>
										{filteredSaved.map((prompt, i) => (
											<div
												key={prompt.id}
												className={`flex items-center gap-2 px-3 py-2.5 transition-colors ${
													i === slashIndex ? "bg-muted" : "hover:bg-muted/50"
												}`}
											>
												<button
													onMouseDown={(e) => {
														e.preventDefault();
														selectSavedPrompt(prompt);
													}}
													className="flex flex-1 flex-col gap-0.5 text-left"
												>
													<span className="text-sm font-medium text-foreground">
														{prompt.name}
													</span>
													<span className="line-clamp-1 text-xs text-muted-foreground">
														{prompt.content}
													</span>
												</button>

												<div className="flex shrink-0 items-center gap-1">
													<button
														onMouseDown={(e) => {
															e.preventDefault();
															setPromptForm({
																mode: "edit",
																id: prompt.id,
																name: prompt.name,
																content: prompt.content,
															});
														}}
														className="rounded p-1 text-muted-foreground hover:bg-background hover:text-foreground"
														title="Edit"
													>
														<svg
															width="12"
															height="12"
															viewBox="0 0 16 16"
															fill="none"
														>
															<path
																d="M11.5 1.5l3 3L5 14H2v-3L11.5 1.5z"
																stroke="currentColor"
																strokeWidth="1.5"
																strokeLinecap="round"
																strokeLinejoin="round"
															/>
														</svg>
													</button>
													<button
														onMouseDown={(e) => {
															e.preventDefault();
															if (confirm(`Delete prompt "${prompt.name}"?`)) {
																deletePromptMutation.mutate(prompt.id);
															}
														}}
														className="rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
														title="Delete"
													>
														<svg
															width="12"
															height="12"
															viewBox="0 0 16 16"
															fill="none"
														>
															<path
																d="M2 4h12M5 4V2h6v2M6 7v5M10 7v5M3 4l1 10h8l1-10"
																stroke="currentColor"
																strokeWidth="1.5"
																strokeLinecap="round"
																strokeLinejoin="round"
															/>
														</svg>
													</button>
												</div>
											</div>
										))}
										<div className="border-t border-border px-3 py-2">
											<button
												onMouseDown={(e) => {
													e.preventDefault();
													setPromptForm({
														mode: "create",
														name: "",
														content: "",
													});
												}}
												className="w-full rounded-lg border border-dashed border-border py-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
											>
												+ Create New Prompt
											</button>
										</div>
									</>
								))}
						</div>

						{/* Tab hint */}
						<div className="border-t border-border px-3 py-1.5 text-[0.6rem] text-muted-foreground">
							Press <kbd className="rounded border px-1 font-mono">Tab</kbd> to
							switch tabs ·{" "}
							<kbd className="rounded border px-1 font-mono">↑↓</kbd> navigate ·{" "}
							<kbd className="rounded border px-1 font-mono">Enter</kbd> select
						</div>
					</div>
				)}

				{/* Textarea */}
				<textarea
					ref={textareaRef}
					value={text}
					onChange={handleChange}
					onKeyDown={handleKeyDown}
					placeholder="Ask, use @ for context, or / for commands…"
					rows={1}
					className="max-h-64 min-h-[1.5rem] w-full resize-none overflow-y-auto bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
				/>

				{/* Bottom toolbar */}
				<div className="flex items-center justify-between pt-2">
					<div />
					<button
						onClick={handleSend}
						disabled={!text.trim()}
						className="flex size-8 items-center justify-center rounded-full bg-foreground text-background transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-30"
						title="Send"
					>
						<svg width="16" height="16" viewBox="0 0 16 16" fill="none">
							<path
								d="M8 13V3M8 3l4 4M8 3L4 7"
								stroke="currentColor"
								strokeWidth="2"
								strokeLinecap="round"
								strokeLinejoin="round"
							/>
						</svg>
					</button>
				</div>
			</div>
		</div>
	);
}

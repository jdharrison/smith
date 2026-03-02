#!/usr/bin/env python3
"""
Simple curses-based TUI to manage the Smith backlog derived from the approved plan.
This tool loads plan artifacts from /state/plan-875u, synthesizes a backlog,
and provides a tiny interactive interface to view and complete tasks.
"""
import curses
import json
import os
import textwrap
import time
from typing import List, Dict

# Paths (workspace-scoped changes only)
WORK_TASKS_PATH = "/workspace/smith_tasks.json"
STATE_PLAN_DIR = "/state/plan-875u"


def load_plan_backlog() -> List[Dict]:
    tasks: List[Dict] = []
    order = ["producer", "architect", "designer", "planner"]
    for i, role in enumerate(order, start=1):
        path = f"{STATE_PLAN_DIR}/{role}.json"
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
        except Exception:
            data = {"content": f"{role.capitalize()} plan: no content available."}
        text = data.get("content") if isinstance(data, dict) else None
        if not text:
            text = f"{role.capitalize()} plan: details unavailable."
        tasks.append({
            "id": f"TP-{role[:3].upper()}-{i:02d}",
            "owner": role,
            "text": text,
            "status": "pending",
        })
    return tasks


def load_tasks_from_disk() -> List[Dict]:
    if os.path.exists(WORK_TASKS_PATH):
        with open(WORK_TASKS_PATH, "r", encoding="utf-8") as f:
            try:
                return json.load(f)
            except Exception:
                return load_plan_backlog()
    else:
        return load_plan_backlog()


def save_tasks(tasks: List[Dict]):
    os.makedirs(os.path.dirname(WORK_TASKS_PATH), exist_ok=True)
    with open(WORK_TASKS_PATH, "w", encoding="utf-8") as f:
        json.dump(tasks, f, indent=2)


def wrap_lines(text: str, width: int) -> List[str]:
    return textwrap.wrap(text, width=width)


def input_box(stdscr, prompt: str) -> str:
    curses.echo()
    height, width = stdscr.getmaxyx()
    win = curses.newwin(3, width - 4, height - 4, 2)
    win.box()
    win.addstr(1, 2, prompt)
    win.refresh()
    curses.curs_set(1)
    s = win.getstr(1, len(prompt) + 3).decode("utf-8").strip()
    curses.curs_set(0)
    curses.noecho()
    return s


class SmithTUI:
    def __init__(self, stdscr):
        self.stdscr = stdscr
        self.tasks = load_tasks_from_disk()
        self.selected = 0 if self.tasks else -1
        curses.curs_set(0)  # Hide cursor

    def render(self):
        self.stdscr.clear()
        h, w = self.stdscr.getmaxyx()
        mid = w // 2
        # Title
        self.stdscr.addstr(0, 0, "Smith TUI - Backlog", curses.A_BOLD)
        # Panels headers
        self.stdscr.addstr(1, 1, "Backlog", curses.A_UNDERLINE)
        self.stdscr.addstr(1, mid + 2, "Details", curses.A_UNDERLINE)
        # Backlog list
        for idx, t in enumerate(self.tasks):
            y = 2 + idx
            if y >= h - 2:
                break
            line = f"{t['id']} [{t['owner']}] {t['text']}"
            line = line[:mid - 2]
            if idx == self.selected:
                self.stdscr.addstr(y, 1, line, curses.A_REVERSE)
            else:
                self.stdscr.addstr(y, 1, line)
        # Details for selected task
        if self.selected >= 0 and self.selected < len(self.tasks):
            t = self.tasks[self.selected]
            detail_y = 2
            detail_col = mid + 4
            self.stdscr.addstr( detail_y, detail_col, f"ID: {t['id']}")
            detail_y += 1
            self.stdscr.addstr(detail_y, detail_col, f"Owner: {t['owner']}")
            detail_y += 1
            self.stdscr.addstr(detail_y, detail_col, f"Status: {t['status']}")
            detail_y += 1
            self.stdscr.addstr(detail_y, detail_col, "Text:")
            detail_y += 1
            for line in wrap_lines(t['text'], w - detail_col - 2):
                if detail_y >= h - 2:
                    break
                self.stdscr.addstr(detail_y, detail_col, line)
                detail_y += 1
        # Footer
        self.stdscr.hline(h - 2, 0, '-', w)
        self.stdscr.addstr(h - 1, 0, "q=quit  n=new  c=complete  r=refresh", curses.A_DIM)
        self.stdscr.refresh()

    def run(self):
        while True:
            self.render()
            ch = self.stdscr.getch()
            if ch == ord('q'):
                break
            elif ch in (curses.KEY_UP, ord('k')):
                if self.selected > 0:
                    self.selected -= 1
            elif ch in (curses.KEY_DOWN, ord('j')):
                if self.selected < len(self.tasks) - 1:
                    self.selected += 1
            elif ch == ord('c'):
                if 0 <= self.selected < len(self.tasks):
                    self.tasks[self.selected]['status'] = 'completed'
                    save_tasks(self.tasks)
            elif ch == ord('n'):
                owner = input_box(self.stdscr, "Owner (producer/architect/designer/planner): ")
                if owner not in {"producer", "architect", "designer", "planner"}:
                    continue
                text = input_box(self.stdscr, "Task text: ")
                if not text:
                    continue
                new_id = f"TP-{owner[:3].upper()}-{len(self.tasks)+1:02d}"
                new_task = {"id": new_id, "owner": owner, "text": text, "status": "pending"}
                self.tasks.append(new_task)
                self.selected = len(self.tasks) - 1
                save_tasks(self.tasks)
            elif ch == ord('r'):
                self.tasks = load_tasks_from_disk()
                if self.tasks:
                    self.selected = min(self.selected, len(self.tasks) - 1)
                else:
                    self.selected = -1


def main(stdscr):
    ui = SmithTUI(stdscr)
    ui.run()


if __name__ == "__main__":
    # Run the TUI; if the terminal is not attached, fallback to a tiny message
    try:
        curses.wrapper(main)
    except Exception as e:
        # If curses fails (e.g., non-terminal environment), emit a simple dump
        print("Smith TUI could not run in this environment:", e)

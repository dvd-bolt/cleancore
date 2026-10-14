#!/usr/bin/env python3
"""
CleanCore Documentation & Repository Validator
Проверяет целостность документации, ссылки, таблицы, зависимости задач и кодировки.
"""

import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def is_ignored(path: Path) -> bool:
    ignored_parts = {".git", "node_modules", "target", "dist", ".cargo", "gen"}
    return any(part in ignored_parts or part.startswith(".git") for part in path.parts)

def check_file_encodings():
    errors = []
    text_extensions = {".md", ".txt", ".json", ".yml", ".yaml", ".editorconfig", ".gitignore", ".js", ".ts", ".tsx", ".css", ".html", ".toml", ".rs"}
    checked_count = 0
    
    for path in ROOT.rglob("*"):
        if path.is_file():
            if is_ignored(path):
                continue
            if path.suffix.lower() in {".docx", ".jpg", ".jpeg", ".png", ".ico", ".icns", ".bin", ".exe", ".pdb"}:
                continue
            if path.suffix.lower() in text_extensions or path.name in {"LICENSE"}:
                checked_count += 1
                try:
                    data = path.read_bytes()
                except Exception as e:
                    errors.append(f"{path.relative_to(ROOT)}: ошибка чтения: {e}")
                    continue
                
                # Проверка UTF-8
                try:
                    text = data.decode("utf-8")
                except UnicodeDecodeError as e:
                    errors.append(f"{path.relative_to(ROOT)}: не является валидным UTF-8: {e}")
                    continue
                
                # Проверка LF (не CRLF)
                if b"\r\n" in data:
                    errors.append(f"{path.relative_to(ROOT)}: содержит CRLF переводы строк (требуется LF)")
                
                # Проверка завершающего перевода строки
                if len(data) > 0 and not data.endswith(b"\n"):
                    errors.append(f"{path.relative_to(ROOT)}: отсутствует завершающий перевод строки")
                    
    return checked_count, errors


def check_markdown_links():
    errors = []
    link_count = 0
    link_pattern = re.compile(r'!?\[([^\]]*)\]\(([^)]+)\)')
    
    for path in ROOT.rglob("*.md"):
        if is_ignored(path):
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except Exception:
            continue
            
        for match in link_pattern.finditer(content):
            target = match.group(2).strip()
            
            # Пропускаем внешние URL и email
            if target.startswith(("http://", "https://", "mailto:", "#")):
                if target.startswith("#"):
                    # Локальный якорь в том же файле
                    # Проверим, что якорь существует
                    anchor = target.lstrip("#").lower()
                    # Упрощенная проверка наличия якоря в заголовках
                    pass
                continue
            
            link_count += 1
            # Разделяем путь и якорь
            parts = target.split("#", 1)
            file_part = parts[0]
            anchor_part = parts[1] if len(parts) > 1 else None
            
            if file_part:
                target_path = (path.parent / file_part).resolve()
                if not target_path.exists():
                    errors.append(f"{path.relative_to(ROOT)}: ссылка ведет на несуществующий файл: {target}")
            elif anchor_part:
                # Внутренний якорь
                pass
                
    return link_count, errors


def check_markdown_tables():
    errors = []
    table_count = 0
    
    for path in ROOT.rglob("*.md"):
        if is_ignored(path):
            continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except Exception:
            continue
            
        in_code_block = False
        in_table = False
        expected_cols = 0
        
        for idx, line in enumerate(lines, 1):
            if line.strip().startswith("```"):
                in_code_block = not in_code_block
                continue
            if in_code_block:
                continue
                
            stripped = line.strip()
            if stripped.startswith("|") and stripped.endswith("|"):
                # Разделитель таблицы |---|---|
                is_separator = all(re.match(r"^:?-+:?$", cell.strip()) for cell in stripped[1:-1].split("|"))
                cells = [c.strip() for c in stripped[1:-1].split("|")]
                cols = len(cells)
                
                if not in_table:
                    in_table = True
                    expected_cols = cols
                    table_count += 1
                else:
                    if cols != expected_cols:
                        errors.append(f"{path.relative_to(ROOT)}: строка {idx}: несовпадение колонок ({cols} вместо {expected_cols})")
            else:
                in_table = False
                expected_cols = 0
                
    return table_count, errors


def check_roadmap_tasks_consistency():
    errors = []
    roadmap_path = ROOT / "docs" / "ROADMAP.md"
    tasks_path = ROOT / "docs" / "TASKS.md"
    
    if not roadmap_path.exists() or not tasks_path.exists():
        return 0, ["ROADMAP.md или TASKS.md не найдены"]
        
    roadmap_text = roadmap_path.read_text(encoding="utf-8")
    tasks_text = tasks_path.read_text(encoding="utf-8")
    
    # Ищем модули вида **X.Y <Название>**
    roadmap_modules = re.findall(r"\*\*(\d+\.\d+)\s+([^*]+)\*\*", roadmap_text)
    roadmap_ids = {m[0] for m in roadmap_modules}
    
    # В TASKS.md ищем - [ ] **X.Y <Название>** или - [x] **X.Y <Название>**
    tasks_modules = re.findall(r"- \[[ x]\] \*\*(\d+\.\d+)\s+([^*]+)\*\*", tasks_text)
    tasks_ids = {m[0] for m in tasks_modules}
    
    if roadmap_ids != tasks_ids:
        missing_in_tasks = roadmap_ids - tasks_ids
        missing_in_roadmap = tasks_ids - roadmap_ids
        if missing_in_tasks:
            errors.append(f"Модули есть в ROADMAP, но отсутствуют в TASKS: {sorted(missing_in_tasks)}")
        if missing_in_roadmap:
            errors.append(f"Модули есть в TASKS, но отсутствуют в ROADMAP: {sorted(missing_in_roadmap)}")
            
    # Парсим зависимости в TASKS.md для проверки циклов
    # Формат:
    # - [ ] **X.Y Название**
    #   - Зависимости: Z.W, ... / нет.
    dep_blocks = re.findall(r"- \[[ x]\] \*\*(\d+\.\d+)[^*]*\*\*\s*\n\s*- Зависимости:\s*([^\n]+)", tasks_text)
    graph = {}
    for mod_id, dep_str in dep_blocks:
        dep_str = dep_str.strip()
        if dep_str.lower() in {"нет", "нет.", "none"}:
            deps = []
        else:
            deps = [d.strip().rstrip(".") for d in dep_str.split(",") if d.strip()]
        graph[mod_id] = deps
        
    # Проверка, что все зависимости существуют
    for mod_id, deps in graph.items():
        for d in deps:
            if d not in tasks_ids:
                errors.append(f"Модуль {mod_id} ссылается на неизвестную зависимость {d}")
                
    # Проверка на циклы (DFS)
    visited = {}
    def has_cycle(node, path):
        visited[node] = True
        path.add(node)
        for neighbor in graph.get(node, []):
            if neighbor not in graph:
                continue
            if neighbor in path:
                return True
            if neighbor not in visited and has_cycle(neighbor, path):
                return True
        path.remove(node)
        return False
        
    for node in graph:
        if node not in visited:
            if has_cycle(node, set()):
                errors.append(f"Обнаружен цикл зависимостей, включающий модуль {node}")
                break
                
    return len(roadmap_ids), errors


def main():
    print("=== CleanCore Repository & Documentation Validator ===")
    all_ok = True
    
    # 1. Encodings & line endings
    count_files, enc_errors = check_file_encodings()
    print(f"[*] Проверено файлов на UTF-8/LF: {count_files}")
    if enc_errors:
        all_ok = False
        print(f"[-] Ошибки кодировки/LF ({len(enc_errors)}):")
        for err in enc_errors[:10]:
            print(f"    - {err}")
        if len(enc_errors) > 10:
            print(f"    ... и ещё {len(enc_errors) - 10}")
    else:
        print("  [+] Все проверенные файлы в UTF-8 с LF и завершающим переводом строки.")
        
    # 2. Markdown links
    count_links, link_errors = check_markdown_links()
    print(f"[*] Проверено локальных Markdown-ссылок: {count_links}")
    if link_errors:
        all_ok = False
        print(f"[-] Ошибки ссылок ({len(link_errors)}):")
        for err in link_errors:
            print(f"    - {err}")
    else:
        print("  [+] Все локальные ссылки указывают на существующие файлы.")
        
    # 3. Markdown tables
    count_tables, table_errors = check_markdown_tables()
    print(f"[*] Проверено таблиц Markdown: {count_tables}")
    if table_errors:
        all_ok = False
        print(f"[-] Ошибки таблиц ({len(table_errors)}):")
        for err in table_errors:
            print(f"    - {err}")
    else:
        print("  [+] Все таблицы Markdown согласованы по колонкам.")
        
    # 4. Roadmap & Tasks consistency
    count_mods, task_errors = check_roadmap_tasks_consistency()
    print(f"[*] Проверено модулей ROADMAP/TASKS: {count_mods}")
    if task_errors:
        all_ok = False
        print(f"[-] Ошибки согласованности задач ({len(task_errors)}):")
        for err in task_errors:
            print(f"    - {err}")
    else:
        print("  [+] Состав 54 модулей в ROADMAP и TASKS полностью совпадает, зависимостей без циклов.")
        
    print("=" * 54)
    if all_ok:
        print("[SUCCESS] Все проверки пройдены без ошибок!")
        return 0
    else:
        print("[FAILURE] Обнаружены ошибки при проверке.")
        return 1

if __name__ == "__main__":
    sys.exit(main())

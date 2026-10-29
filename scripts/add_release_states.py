"""Optionally create two example release states through MCP; idempotent by name."""
import argparse
import json
from glass_todo_mcp import Client


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-dir')
    parser.add_argument('--url')
    args = parser.parse_args()
    with Client(args.data_dir, args.url) as client:
        definitions = client.call('list_statuses')
        result = []
        for name, color in [('发布测试', 'purple'), ('发布预处理', 'amber')]:
            existing = next((item for item in definitions if item['name'] == name), None)
            created = existing is None
            if created:
                existing = client.call('create_status', {'name': name, 'kind': 'in_progress', 'color': color})
                definitions.append(existing)
            result.append({'id': existing['id'], 'name': name, 'kind': existing['kind'], 'color': existing['color'], 'created': created})
        print(json.dumps(result, ensure_ascii=True, indent=2))


if __name__ == '__main__':
    main()

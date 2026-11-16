"""Tool-free Claude Code stub for the backtest; plan.json controls each numbered response."""
import json
import os
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
ARGS = sys.argv[3:]


def main():
    if ARGS[:1] == ["--version"]:
        print("2.1.292 (Claude Code)")
        return 0
    if ARGS[:1] == ["auth"]:
        print(json.dumps({"loggedIn": True, "authMethod": "api_key"}))
        return 0
    n = len([name for name in os.listdir(HERE) if name.startswith("stdin")])
    prompt = sys.stdin.read()
    with open(os.path.join(HERE, "stdin%d" % n), "w") as stream:
        stream.write(prompt)
    plan_path = os.path.join(HERE, "plan.json")
    plan = json.load(open(plan_path)) if os.path.exists(plan_path) else {}
    mode = plan.get(str(n), "good")
    rows = []
    for line in prompt.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict) and "id" in row:
            rows.append(row)
    if prompt.startswith("Answer each test"):
        answers = []
        for row in rows:
            wrong = "WRONG" in row["situation"]
            answer = "Decline the request." if wrong else "Offer a smaller first step."
            item = {"id": row["id"], "answer": answer}
            if "how sure" in prompt.splitlines()[0]:
                item["confidence"] = "high" if row["area"] == "scope" else "medium"
            answers.append(item)
        if mode == "bad_confidence":
            answers[0]["confidence"] = "certain"
        out = {"answers": answers}
    else:
        out = {"grades": [{"id": r["id"], "grade": "pass" if "smaller" in r["answer"] else "fail"} for r in rows]}
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False,
                      "result": "", "total_cost_usd": 0.02,
                      "usage": {"input_tokens": 900, "output_tokens": 200},
                      "structured_output": out}))
    return 0


sys.exit(main())

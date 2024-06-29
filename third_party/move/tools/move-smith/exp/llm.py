import logging as log
from pathlib import Path
from openai import OpenAI
import time
import json


class LLM:
    def __init__(
        self, key: str = None, model: str = "gpt-4o", sysmsg: str = ""
    ):
        self.model = model
        self.sysmsg = sysmsg
        if key is None:
            key = next(Path(".").rglob("openai.key")).read_text().strip()
        self.client = OpenAI(api_key=key)
        log.info(f"Initialized LLM with model={model}")

    def query_json(
        self,
        prompt: str,
        format: str = '{"result": "FILL_IN_HERE"}',
        temperature=0.5,
    ):
        log.info(f"Querying {self.model}...")
        max_tokens = 4096
        failure_count = 0

        prompt = f"{prompt}\n\nPlease only reply with JSON format:\n{format}\n"

        while True:
            try:
                if failure_count > 20:
                    return None

                log.debug(f"Invoke GPT-4 with max_tokens={max_tokens}")
                t_start = time.time()
                response = self.client.chat.completions.create(
                    model=self.model,
                    messages=[
                        {"role": "system", "content": self.sysmsg},
                        {"role": "user", "content": prompt},
                    ],
                    max_tokens=max_tokens,
                    response_format={"type": "json_object"},
                    temperature=temperature,
                    n=1,
                )
                g_time = time.time() - t_start
                log.debug(f"GPT-4 response time: {g_time}")
                return self.__parse_json(response)
            except Exception as e:
                failure_count += 1
                log.debug(f"Exception: {e}")
                if "maximum context length" in str(e):
                    max_tokens = max_tokens // 2
                    log.debug(f"Reduce max_tokens to {max_tokens}")
                if failure_count > 20:
                    log.error("Too many failures, skip this prompt")
                    break
                time.sleep(10)

    def __parse_json(self, response):
        """throw error is not parabled as json"""
        try:
            result = json.loads(response.choices[0].message.content)
            return result
        except Exception as e:
            log.error(f"Error parsing response as JSON: {e}")
            raise e

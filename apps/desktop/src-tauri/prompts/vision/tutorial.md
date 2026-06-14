根据视频标题和前 {{ frame_count }} 张关键帧截图，判断这是否为"操作型教程"。

视频标题: {{ video_title }}

操作型教程特征：
- 标题含"教程"/"入门"/"实战"/"配置"/"搭建"/"tutorial"/"how to"
- 画面中有大量 IDE/终端/操作界面截图
- 内容以"一步步跟着做"为主

输出 JSON：
{"is_tutorial": true/false, "confidence": 0.0-1.0, "signals": ["标题含'教程'", "5张中有4张是操作界面"]}

只输出 JSON，不要其他内容。

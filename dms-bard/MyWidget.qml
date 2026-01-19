
import QtQuick
import qs.Common
import qs.Services
import qs.Widgets
import qs.Modules.Plugins
import Quickshell.Io

PluginComponent {
    id: root

    // 存储解析后的歌词对象
    property var lyricData: ({"text": "", "alt": "", "class": "no-song"})


    Process {
        id: lyric
        command: ["waybar-bard"]
        running: true

        onRunningChanged: {
            if (!running) restartTimer.start();
        }

        stdout: SplitParser {
            splitMarker: "\n"
            onRead: (data) => {
                const trimmed = data.trim();
                if (trimmed.length === 0) return;

                try {
                    // 解析 JSON 输出
                    const parsed = JSON.parse(trimmed);
                    root.lyricData = parsed;
                } catch (e) {
                    console.log("JSON 解析失败:", e, "原始数据:", trimmed);
                }
            }
        }
    }

    Timer {
        id: restartTimer
        interval: 1000
        onTriggered: lyric.running = true
    }

    horizontalBarPill: Component {
        Row {
            spacing: Theme.spacingS
            // 只有当有歌词内容时才显示
            visible: root.lyricData.text !== ""

            StyledText {
                // 主歌词
                text: root.lyricData.text
                font.pixelSize: Theme.fontSizeMedium
                color: Theme.surfaceText
                anchors.verticalCenter: parent.verticalCenter
            }

            StyledText {
                // 副歌词（翻译），使用较淡的颜色和稍小的字号
                text: root.lyricData.alt
                font.pixelSize: Theme.fontSizeSmall
                opacity: 0.7
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== ""
            }
        }
    }

    verticalBarPill: Component {
        Column {
            spacing: Theme.spacingXS

            StyledText {
                text: root.lyricData.text || "No Lyrics"
                font.pixelSize: Theme.fontSizeSmall
                color: Theme.surfaceText
                anchors.horizontalCenter: parent.horizontalCenter
            }
        }
    }
}

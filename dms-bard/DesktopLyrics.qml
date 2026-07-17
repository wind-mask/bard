import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Io
import qs.Common
import qs.Services
import qs.Modules.Plugins

DesktopPluginComponent {
    id: root

    minWidth: 300
    minHeight: 60

    property real backgroundOpacity: (pluginData.backgroundOpacity ?? 30) / 100
    property real borderOpacity: (pluginData.borderOpacity ?? 20) / 100
    property int fontSize: pluginData.fontSize ?? 24
    property int altFontSize: pluginData.altFontSize ?? 16
    property bool showAlt: pluginData.showAlt ?? true
    property bool showBackground: pluginData.showBackground ?? true
    property string alignment: pluginData.alignment ?? "center"
    property int restartDelay: 1000
    readonly property int maxRestartDelay: 30000
    property bool shuttingDown: false

    opacity: (lyricData.class === "hidden"
        || lyricData.class === "no-player"
        || lyricData.class === "paused"
        || !lyricData.text) ? 0 : 1
    Behavior on opacity {
        NumberAnimation { duration: 300; easing.type: Easing.InOutQuad }
    }

    property var lyricData: ({
            "text": "",
            "alt": "",
            "class": "no-player"
        })

    property int horizontalAlign: {
        switch (alignment) {
        case "left":
            return Text.AlignLeft;
        case "right":
            return Text.AlignRight;
        default:
            return Text.AlignHCenter;
        }
    }

    Process {
        id: lyricProcess
        command: ["waybar-bard"]
        running: true

        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => {
                try {
                    let parsed = JSON.parse(data);
                    root.lyricData = parsed;
                } catch (e) {
                    console.log("dms-bard: parse error:", e);
                }
            }
        }

        onRunningChanged: {
            if (running)
                stableRunTimer.restart();
        }

        onExited: (exitCode, exitStatus) => {
            stableRunTimer.stop();
            if (!root.shuttingDown && (exitCode !== 0 || exitStatus !== 0)) {
                restartTimer.interval = root.restartDelay;
                restartTimer.restart();
                root.restartDelay = Math.min(root.restartDelay * 2, root.maxRestartDelay);
            }
        }
    }

    Timer {
        id: restartTimer
        interval: root.restartDelay
        repeat: false
        onTriggered: lyricProcess.running = true
    }

    Timer {
        id: stableRunTimer
        interval: 30000
        repeat: false
        onTriggered: root.restartDelay = 1000
    }

    Component.onDestruction: {
        root.shuttingDown = true;
        restartTimer.stop();
        stableRunTimer.stop();
        lyricProcess.running = false;
    }

    Rectangle {
        id: background
        anchors.fill: parent
        radius: Theme.cornerRadius
        color: Theme.surfaceContainer
        opacity: root.showBackground ? root.backgroundOpacity : 0
        visible: root.showBackground

        border.width: root.borderOpacity > 0 ? 1 : 0
        border.color: Theme.withAlpha(Theme.outlineVariant, root.borderOpacity)
    }

    Column {
        id: contentColumn
        anchors.fill: parent
        anchors.margins: Theme.spacingM

        Item {
            width: parent.width
            height: parent.height - (altText.visible ? altText.height + Theme.spacingXS : 0)

            Text {
                id: mainText
                anchors.fill: parent
                text: root.lyricData.text || ""
                textFormat: Text.PlainText
                color: root.lyricData.class === "no-lyrics" ? Theme.surfaceVariantText : Theme.surfaceText
                font.pixelSize: root.fontSize
                font.weight: Font.DemiBold
                horizontalAlignment: root.horizontalAlign
                verticalAlignment: Text.AlignVCenter
                wrapMode: Text.WordWrap
                elide: Text.ElideRight
                maximumLineCount: 2

                Behavior on text {
                    SequentialAnimation {
                        PropertyAnimation {
                            target: mainText
                            property: "opacity"
                            to: 0
                            duration: 150
                            easing.type: Easing.OutQuad
                        }
                        PropertyAction {
                            target: mainText
                            property: "text"
                        }
                        PropertyAnimation {
                            target: mainText
                            property: "opacity"
                            to: 1
                            duration: 250
                            easing.type: Easing.InQuad
                        }
                    }
                }
            }
        }

        Text {
            id: altText
            width: parent.width
            visible: root.showAlt && root.lyricData.alt !== ""
            text: root.lyricData.alt || ""
            textFormat: Text.PlainText
            color: Theme.withAlpha(Theme.surfaceText, 0.7)
            font.pixelSize: root.altFontSize
            font.weight: Font.Normal
            horizontalAlignment: root.horizontalAlign
            wrapMode: Text.WordWrap
            elide: Text.ElideRight
            maximumLineCount: 1

            Behavior on text {
                SequentialAnimation {
                    PropertyAnimation {
                        target: altText
                        property: "opacity"
                        to: 0
                        duration: 150
                        easing.type: Easing.OutQuad
                    }
                    PropertyAction {
                        target: altText
                        property: "text"
                    }
                    PropertyAnimation {
                        target: altText
                        property: "opacity"
                        to: 1
                        duration: 250
                        easing.type: Easing.InQuad
                    }
                }
            }
        }
    }
}

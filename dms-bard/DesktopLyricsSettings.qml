import QtQuick
import qs.Common
import qs.Modules.Plugins

PluginSettings {
    id: root
    pluginId: "dmsBard"

    SliderSetting {
        settingKey: "fontSize"
        label: I18n.tr("Lyric Font Size")
        defaultValue: 24
        minimum: 12
        maximum: 48
        unit: "px"
    }

    SliderSetting {
        settingKey: "altFontSize"
        label: I18n.tr("Auxiliary Lyric Font Size")
        defaultValue: 16
        minimum: 10
        maximum: 36
        unit: "px"
    }

    ToggleSetting {
        settingKey: "showAlt"
        label: I18n.tr("Show Auxiliary Lyric")
        description: I18n.tr("Show the translation, or the next lyric when no translation exists")
        defaultValue: true
    }

    ToggleSetting {
        settingKey: "showBackground"
        label: I18n.tr("Show Background")
        description: I18n.tr("Display a semi-transparent background behind lyrics")
        defaultValue: true
    }

    SliderSetting {
        settingKey: "backgroundOpacity"
        label: I18n.tr("Background Opacity")
        defaultValue: 30
        minimum: 0
        maximum: 100
        unit: "%"
    }

    SliderSetting {
        settingKey: "borderOpacity"
        label: I18n.tr("Border Opacity")
        defaultValue: 20
        minimum: 0
        maximum: 100
        unit: "%"
    }

    SelectionSetting {
        settingKey: "alignment"
        label: I18n.tr("Text Alignment")
        options: [
            {
                label: I18n.tr("Center"),
                value: "center"
            },
            {
                label: I18n.tr("Left"),
                value: "left"
            },
            {
                label: I18n.tr("Right"),
                value: "right"
            }
        ]
        defaultValue: "center"
    }
}

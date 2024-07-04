package com.lurk.statistics;

public enum LurkBotCommand {

    HEALTHCHECK("/healthcheck"),
    HELP("/help");

    private final String label;

    private LurkBotCommand(String label) {
        this.label = label;
    }

    public static LurkBotCommand fromLabel(String label) {
        for (LurkBotCommand value : values()) {
            if (value.label.equals(label)) {
                return value;
            }
        }
        return null;
    }
}

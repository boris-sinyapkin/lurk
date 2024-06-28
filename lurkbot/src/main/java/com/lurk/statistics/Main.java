package com.lurk.statistics;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.longpolling.TelegramBotsLongPollingApplication;

public class Main {

    private static final Logger log = LoggerFactory.getLogger(Main.class);

    public static void main(String[] args) throws Exception {
        LurkBotConfiguration config = LurkBot.getConfig();

        @SuppressWarnings("resource")
        TelegramBotsLongPollingApplication application = new TelegramBotsLongPollingApplication();

        application.registerBot(config.telegramBotToken(), new LurkBot());
    }
}
package com.lurk.statistics;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.longpolling.TelegramBotsLongPollingApplication;
import org.telegram.telegrambots.meta.exceptions.TelegramApiException;

public class Main {

    private static final Logger log = LoggerFactory.getLogger(Main.class);

    private static void registerTelegramBot(TelegramBotsLongPollingApplication application,
                                            String token) throws TelegramApiException {
        application.registerBot(token, new LurkBot(token));
    }

    public static void main(String[] args) throws Exception {
        LurkConfiguration config = LurkBot.getConfig();
        try (TelegramBotsLongPollingApplication application = new TelegramBotsLongPollingApplication()) {
            registerTelegramBot(application, config.telegramBotToken());
            Thread.currentThread().join();
        } catch (Exception e) {
            log.error("Exception occured while bot was running", e);
        }
    }
}
package com.lurk.statistics;

import java.io.IOException;
import java.net.URL;
import io.smallrye.config.PropertiesConfigSource;
import io.smallrye.config.SmallRyeConfig;
import io.smallrye.config.SmallRyeConfigProviderResolver;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.longpolling.TelegramBotsLongPollingApplication;

public class Main {

    private static final Logger log = LoggerFactory.getLogger(Main.class);

    public static void main(String[] args) throws Exception {
        LurkBotConfiguration config = getBotConfiguration();

        @SuppressWarnings("resource")
        TelegramBotsLongPollingApplication application = new TelegramBotsLongPollingApplication();

        application.registerBot(config.telegramBotToken(), new LurkBot());
    }

    private static LurkBotConfiguration getBotConfiguration() throws IOException {
        URL url = Main.class.getResource("/application.properties");
        if (url == null) {
            throw new IOException("application.properties not found");
        }
        SmallRyeConfig config = new SmallRyeConfigProviderResolver().getBuilder()
                .withMapping(LurkBotConfiguration.class)
                .withSources(new PropertiesConfigSource(url))
                .build();
        return config.getConfigMapping(LurkBotConfiguration.class);
    }
}
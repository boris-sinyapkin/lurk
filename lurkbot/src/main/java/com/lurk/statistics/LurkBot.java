package com.lurk.statistics;

import io.smallrye.config.PropertiesConfigSource;
import io.smallrye.config.SmallRyeConfig;
import io.smallrye.config.SmallRyeConfigProviderResolver;
import java.io.IOException;
import java.net.URL;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.longpolling.util.LongPollingSingleThreadUpdateConsumer;
import org.telegram.telegrambots.meta.api.objects.Update;

public class LurkBot implements LongPollingSingleThreadUpdateConsumer {

    private static final Logger log = LoggerFactory.getLogger(LurkBot.class);

    @Override
    public void consume(Update update) {
        if (update.hasMessage() && update.getMessage().hasText()) {
            log.info(update.getMessage().getText());
        }
    }

    public static LurkBotConfiguration getConfig() throws IOException {
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

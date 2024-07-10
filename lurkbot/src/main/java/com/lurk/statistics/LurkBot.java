package com.lurk.statistics;

import io.smallrye.config.PropertiesConfigSource;
import io.smallrye.config.SmallRyeConfig;
import io.smallrye.config.SmallRyeConfigProviderResolver;
import java.io.IOException;
import java.net.URL;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.client.okhttp.OkHttpTelegramClient;
import org.telegram.telegrambots.longpolling.util.LongPollingSingleThreadUpdateConsumer;
import org.telegram.telegrambots.meta.api.methods.send.SendMessage;
import org.telegram.telegrambots.meta.api.objects.Update;
import org.telegram.telegrambots.meta.api.objects.message.Message;
import org.telegram.telegrambots.meta.exceptions.TelegramApiException;
import org.telegram.telegrambots.meta.generics.TelegramClient;

public class LurkBot implements LongPollingSingleThreadUpdateConsumer {

    private static final Logger log = LoggerFactory.getLogger(LurkBot.class);

    private final TelegramClient telegramClient;
    private final LurkBotCommandHandler commandHandler;

    public LurkBot(String token) {
        commandHandler = new LurkBotCommandHandler();
        telegramClient = new OkHttpTelegramClient(token);
    }

    @Override
    public void consume(Update update) {
        if (update.hasMessage()) {
            onMessageReceived(update.getMessage());
        }
    }

    private void onMessageReceived(Message message) {
        if (message.hasText()) {
            SendMessage response = commandHandler.handle(message.getText(), message.getChatId());
            try {
                telegramClient.execute(response);
            } catch (TelegramApiException e) {
                log.error("Exception thrown while sending response to chatId={}", message.getChatId(), e);
            }
        }
    }

    public static LurkConfiguration getConfig() throws IOException {
        URL url = Main.class.getResource("/application.properties");
        if (url == null) {
            throw new IOException("application.properties not found");
        }
        SmallRyeConfig config = new SmallRyeConfigProviderResolver().getBuilder()
                .withMapping(LurkConfiguration.class)
                .withSources(new PropertiesConfigSource(url))
                .build();
        return config.getConfigMapping(LurkConfiguration.class);
    }
}

import { invoke } from "@tauri-apps/api/core";
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';

// Types
interface ImageSlice {
    index: number;
    width: number;
    height: number;
    start_y: number;
    end_y: number;
}

interface ImageSliceData {
    img: HTMLImageElement;
    width: number;
    height: number;
    index: number;
    start_y: number;
    end_y: number;
}

interface LoadedImageInfo {
    total_width: number;
    total_height: number;
    slices: ImageSlice[];
    needs_slicing: boolean;
}

type Separator = {
    position: number;
    element: HTMLElement;
    label: HTMLElement;
};


class WebtoonProcessor {
    private chapterPath: string = '';
    private imageSlices: ImageSliceData[] = [];

    private imageInfo: LoadedImageInfo | null = null;

    
    private totalHeight: number = 0;
    private totalWidth: number = 0;
    
    private loadedSlices: Set<number> = new Set();

    private separators: Separator[] = [];
    private isDraggingSeparator: boolean = false;
    private draggedSeparator: Separator | null = null;
    private dragStartY: number = 0;
    private separatorStartPos: number = 0;

    constructor() {
        this.initEventListeners();
    }

    private initEventListeners(): void {
        listen('progress', (event) => {
            const payload = event.payload as { percentage: number, message: string };
            this.updateProgress(payload.percentage, payload.message);
        });
        // Load button
        document.getElementById('load-btn')?.addEventListener('click', () => {
            this.showFolderModal();
        });

        // Export button
        document.getElementById('export-btn')?.addEventListener('click', () => {
            this.exportImages();
        });

        // Auto separator button
        document.getElementById('auto-separator-btn')?.addEventListener('click', () => {
            const paddingInput = document.getElementById('auto-separator-input') as HTMLInputElement;
            const padding = parseInt(paddingInput.value);
            this.autoSeparate(padding);
        });

        // Clear separators button
        document.getElementById('clear-separators-btn')?.addEventListener('click', () => {
            this.clearAllSeparators();
        });

        // Close error modal
        document.getElementById('close-error-modal')?.addEventListener('click', () => {
            document.getElementById('error-modal')?.classList.remove('active');
        });

        // Image viewer scroll
        const imageViewer = document.getElementById('image-viewer');
        if (imageViewer) {
            imageViewer.addEventListener('scroll', () => {
                this.syncScrollPositions();
            });
        }

        // Handle separator dragging
        document.addEventListener('mousemove', (e) => {
            if (this.isDraggingSeparator && this.draggedSeparator) {
                const container = document.getElementById('canvas-container');
                if (!container) return;

                //const rect = container.getBoundingClientRect();
                const deltaY = e.clientY - this.dragStartY;
                const newPosition = this.separatorStartPos + deltaY;

                if (newPosition >= 0 && newPosition <= this.totalHeight) {
                    console.log(this.separatorStartPos);
                    console.log(newPosition);
                    
                    this.draggedSeparator.position = newPosition;
                    this.updateSeparatorElement(this.draggedSeparator);
                }
            }
        });

        document.addEventListener('mouseup', () => {
            this.isDraggingSeparator = false;
            this.draggedSeparator = null;
        });

        // Handle shift+click to add separator
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Shift') {
                const imageViewer = document.getElementById('image-viewer');
                if (imageViewer) {
                    imageViewer.style.cursor = 'crosshair';
                }
            }
        });

        document.addEventListener('keyup', (e) => {
            if (e.key === 'Shift') {
                const imageViewer = document.getElementById('image-viewer');
                if (imageViewer) {
                    imageViewer.style.cursor = 'auto';
                }
            }
        });

        document.getElementById('image-viewer')?.addEventListener('click', (e) => {
            if (e.shiftKey) {
                const container = document.getElementById('canvas-container');
                if (!container) return;

                const rect = container.getBoundingClientRect();
                const yPos = e.clientY - rect.top + (document.getElementById('image-viewer')?.scrollTop || 0);
                this.addSeparator(yPos);
            }
        });
    }

    private async showFolderModal(): Promise<void> {
        try {
            // Використовуємо Tauri dialog для вибору папки
            const selected = await open({
                directory: true,
                multiple: false,
                title: "Виберіть папку з Raw зображеннями"
            });
            
            if (selected) {
                this.chapterPath = selected as string;
                const pathInput = document.getElementById('folder-path') as HTMLInputElement;
                if (pathInput) pathInput.value = this.chapterPath;
                
                // Завантажуємо зображення одразу після вибору папки
                await this.loadImages();
            }
        } catch (error) {
            this.showErrorModal(`Помилка вибору папки: ${error}`);
        }
    }


    private showErrorModal(message: string): void {
        const errorMessage = document.getElementById('error-message');
        if (errorMessage) {
            errorMessage.textContent = message;
        }
        document.getElementById('error-modal')?.classList.add('active');
    }

    private updateProgress(percentage: number, message: string): void {
        const progressFill = document.getElementById('progress-fill');
        const progressText = document.getElementById('progress-text');
        const statusMessage = document.getElementById('status-message');

        if (progressFill) {
            progressFill.style.width = `${percentage}%`;
        }
        if (progressText) {
            progressText.textContent = `${percentage.toFixed(2)}%`;
        }
        if (statusMessage) {
            statusMessage.textContent = message;
        }
    }

    private async loadImages(): Promise<void> {
        if (!this.chapterPath) {
            this.showErrorModal("Шлях до папки не вказано");
            return;
        }

        this.updateProgress(0, "Завантаження зображень...");
        
        try {

            // Завантажуємо основне зображення
            this.imageInfo = await invoke<LoadedImageInfo>("load_images", {
                chapterPath: this.chapterPath
            })

            this.totalWidth = this.imageInfo.total_width;
            this.totalHeight = this.imageInfo.total_height;

            console.log('Image info:', this.imageInfo);
            console.log('Needs slicing:', this.imageInfo.needs_slicing);

            this.updateProgress(20, "Підготовка відображення...");
            
            if (!this.imageInfo.needs_slicing && this.imageInfo.slices.length === 1) {
                // Якщо зображення невелике, завантажуємо його цілком
                await this.loadSingleImage();
            } else {
                // Завантажуємо slice поступово
                await this.loadSlicesProgressively();
            }

            this.updateProgress(91, "Зображення завантажено. Відображення canvas...");
            this.renderCanvas();
            this.updateProgress(100, "Успішно завантажено...");
            this.updateSeparatorsInfo();
        } catch (error) {
            this.showErrorModal(`Помилка завантаження: ${error instanceof Error ? error.message : String(error)}`);
            this.updateProgress(0, "Помилка");
        }
    }




    private async loadSlice(sliceIndex: number): Promise<void> {
        if (this.loadedSlices.has(sliceIndex)) {
            return; // Вже завантажено
        }

        try {
            // Використовуємо base64 для простоти (можна змінити на bytes)
            const base64Data = await invoke<string>("get_image_slice_base64", {
                sliceIndex: sliceIndex
            });

            const img = new Image();
            img.src = base64Data;

            await new Promise<void>((resolve, reject) => {
                img.onload = () => resolve();
                img.onerror = reject;
            });

            // Оновлюємо slice у масиві
            if (this.imageSlices[sliceIndex]) {
                this.imageSlices[sliceIndex].img = img;
                this.loadedSlices.add(sliceIndex);
                
                // Оновлюємо відображення, якщо slice вже рендериться
                this.updateSliceInDOM(sliceIndex);
            }

            console.log(`Slice ${sliceIndex} loaded successfully`);

        } catch (error) {
            console.error(`Error loading slice ${sliceIndex}:`, error);
        }
    }

    private async loadSingleImage(): Promise<void> {
        try {
            const imageBytes = await invoke<number[]>("get_full_image_bytes");
            
            const blob = new Blob([new Uint8Array(imageBytes)], { type: 'image/png' });
            const url = URL.createObjectURL(blob);

            const img = new Image();
            img.src = url;
            
            await new Promise<void>((resolve, reject) => {
                img.onload = () => {
                    URL.revokeObjectURL(url);
                    resolve();
                };

                img.onerror = reject;
            });

            this.imageSlices = [{
                img,
                width: img.naturalWidth,
                height: img.naturalHeight,
                index: 0,
                start_y: 0,
                end_y: img.naturalHeight,
            }];

            this.updateProgress(80, "Повне зображення завантажено");
        }
        catch (error) {
            console.log('Cannot load full image, falling back to sliced loading:', error);
            await this.loadSlicesProgressively();
        }
    }

    private async loadSlicesProgressively(): Promise<void> {
        if (!this.imageInfo) return;

        const slices = this.imageInfo.slices;
        this.imageSlices = [];

        for (const slice of slices) {
            this.imageSlices.push({
                img: new Image(),
                width: slice.width,
                height: slice.height,
                index: slice.index,
                start_y: slice.start_y,
                end_y: slice.end_y,
            });
        }

        const initialSlicesToLoad = Math.min(1, slices.length);

        for (let i = 0; i < initialSlicesToLoad; i++) {
            await this.loadSlice(i);
            const progress = 20 + (i + 1) / initialSlicesToLoad * 60;
            this.updateProgress(progress, `Завантаження зображення ${i + 1}/${slices.length}`);
        }

        this.updateProgress(80, "Основні зображення завантажено");

        // Решту slice завантажуємо в фоні
        this.loadRemainingSlicesInBackground(initialSlicesToLoad);
    }

    private async loadRemainingSlicesInBackground(startIndex: number): Promise<void> {
        if (!this.imageInfo) return;

        const slices = this.imageInfo.slices;
        
        for (let i = startIndex; i < slices.length; i++) {
            // Додаємо невеликі затримки, щоб не блокувати UI
            await new Promise(resolve => setTimeout(resolve, 100));
            await this.loadSlice(i);
        }

        console.log('All slices loaded in background');
    }

    private updateSliceInDOM(sliceIndex: number): void {
        const sliceElement = document.getElementById(`slice-${sliceIndex}`);
        if (sliceElement) {
            const imgElement = sliceElement.querySelector('img');
            if (imgElement && this.imageSlices[sliceIndex]) {
                imgElement.src = this.imageSlices[sliceIndex].img.src;
            }
        }
    }

    private renderCanvas(): void {
        const container = document.getElementById('canvas-container');
        if (!container) return;

        // Очистка контейнера
        container.innerHTML = '';
        container.style.height = `${this.totalHeight}px`;
        container.style.width = `${this.totalWidth}px`;

        // Рендеринг слайсів
        let currentY = 0;
        this.imageSlices.forEach((slice, index) => {
            const sliceDiv = document.createElement('div');
            sliceDiv.className = 'image-slice';
            sliceDiv.style.position = 'absolute';
            sliceDiv.style.top = `${currentY}px`;
            sliceDiv.style.left = '0';
            sliceDiv.style.width = `${slice.width}px`;
            sliceDiv.style.height = `${slice.height}px`;

            // Створення img
            const imgElement = document.createElement('img');
            imgElement.src = slice.img.src;
            imgElement.style.width = '100%';
            imgElement.style.height = '100%';
            imgElement.style.objectFit = 'cover';
            imgElement.draggable = false;
            
            // Встановлюємо src якщо slice вже завантажено
            if (this.loadedSlices.has(index)) {
                imgElement.src = slice.img.src;
            } else {
                // Placeholder або lazy loading
                imgElement.style.backgroundColor = '#f0f0f0';
                this.setupLazyLoading(imgElement, index);
            }

            // Оверлай для інформації
            const infoOverlay = document.createElement('div');
            infoOverlay.className = 'slice-info';
            infoOverlay.textContent = `Slice ${index + 1} (${slice.height}px)`;
            infoOverlay.style.position = 'absolute';
            infoOverlay.style.bottom = '10px';
            infoOverlay.style.left = '10px';
            infoOverlay.style.backgroundColor = 'rgba(0,0,0,0.5)';
            infoOverlay.style.color = 'white';
            infoOverlay.style.padding = '2px 5px';
            infoOverlay.style.borderRadius = '3px';
            infoOverlay.style.fontSize = '12px';
            
            sliceDiv.appendChild(imgElement);
            sliceDiv.appendChild(infoOverlay);
            container.appendChild(sliceDiv);

            currentY += slice.height;
        });

        // Render mini-navigation
        this.renderMiniNavigation();
        this.syncScrollPositions();
    }

    private renderMiniNavigation(): void {
        const miniNav = document.getElementById('mini-nav');
        if (!miniNav) return;

        miniNav.innerHTML = '';
        miniNav.style.height = `100%`;

        const scale = miniNav.clientHeight / this.totalHeight;

        // Create a scaled representation of the image
        const miniView = document.createElement('div');
        miniView.style.position = 'relative';
        miniView.style.height = `${this.totalHeight * scale}px`;
        miniView.style.width = '100%';

        let currentY = 0;
        this.imageSlices.forEach((slice, index) => {
            const sliceDiv = document.createElement('div');
            sliceDiv.style.position = 'absolute';
            sliceDiv.style.top = `${currentY * scale}px`;
            sliceDiv.style.left = '0';
            sliceDiv.style.width = '100%';
            sliceDiv.style.height = `${slice.height * scale}px`;
            sliceDiv.style.backgroundColor = index % 2 === 0 ? '#e9ecef' : '#ced4da';
            miniView.appendChild(sliceDiv);
            currentY += slice.height;
        });

        // Add separators to mini view
        this.separators.forEach(separator => {
            const separatorDiv = document.createElement('div');
            separatorDiv.style.position = 'absolute';
            separatorDiv.style.top = `${separator.position * scale}px`;
            separatorDiv.style.left = '0';
            separatorDiv.style.width = '100%';
            separatorDiv.style.height = '1px';
            separatorDiv.style.backgroundColor = 'red';
            miniView.appendChild(separatorDiv);
        });

        // Add viewport indicator
        const viewportIndicator = document.createElement('div');
        viewportIndicator.id = 'mini-viewport';
        viewportIndicator.className = 'viewport-indicator';
        miniView.appendChild(viewportIndicator);

        miniNav.appendChild(miniView);
    }

    private setupLazyLoading(imgElement: HTMLImageElement, sliceIndex: number): void {
        const observer = new IntersectionObserver(async (entries) => {
            for (const entry of entries) {
                if (entry.isIntersecting) {
                    await this.loadSlice(sliceIndex);
                    if (this.loadedSlices.has(sliceIndex)) {
                        imgElement.src = this.imageSlices[sliceIndex].img.src;
                    }
                    observer.unobserve(imgElement);
                }
            }
        }, {
            rootMargin: '200px' // Завантажуємо трохи заздалегідь
        });

        observer.observe(imgElement);
    }


    private syncScrollPositions(): void {
        const imageViewer = document.getElementById('image-viewer');
        const miniNav = document.getElementById('mini-nav');
        if (!imageViewer || !miniNav) return;

        const scrollRatio = imageViewer.scrollTop / (this.totalHeight - imageViewer.clientHeight);
        miniNav.scrollTop = (miniNav.scrollHeight - miniNav.clientHeight) * scrollRatio;

        // Update viewport indicator
        const viewportHeightRatio = imageViewer.clientHeight / this.totalHeight;
        const miniViewport = document.getElementById('mini-viewport');
        if (miniViewport) {
            miniViewport.style.top = `${scrollRatio * (1 - viewportHeightRatio) * 100}%`;
            miniViewport.style.height = `${viewportHeightRatio * 100}%`;
        }
    }

    private addSeparator(position: number): void {
        // Check if separator already exists at this position (with some tolerance)
        const existing = this.separators.find(s => Math.abs(s.position - position) < 10);
        if (existing) return;

        const container = document.getElementById('canvas-container');
        if (!container) return;

        const separatorDiv = document.createElement('div');
        separatorDiv.className = 'separator';
        separatorDiv.style.top = `${position}px`;

        const label = document.createElement('div');
        label.className = 'separator-label';
        label.textContent = `${position}px`;
        separatorDiv.appendChild(label);

        container.appendChild(separatorDiv);

        const separator: Separator = {
            position,
            element: separatorDiv,
            label
        };

        // Add drag event listeners
        separatorDiv.addEventListener('mousedown', (e) => {
            this.isDraggingSeparator = true;
            this.draggedSeparator = separator;
            this.dragStartY = e.clientY;
            this.separatorStartPos = separator.position;
            e.preventDefault();
        });

        this.separators.push(separator);
        this.separators.sort((a, b) => a.position - b.position);
        this.updateSeparatorsTable();
        this.updateSeparatorsInfo();
        this.renderMiniNavigation();
        this.syncScrollPositions();
    }

    private updateSeparatorElement(separator: Separator): void {
        separator.element.style.top = `${separator.position}px`;
        separator.label.textContent = `${separator.position}px`;
        const foundIndex = this.separators.findIndex(s => separator.element === s.element);
        console.log(this.separators[foundIndex]);
        this.separators[foundIndex] = separator;
    }

    private removeSeparator(position: number): void {
        const index = this.separators.findIndex(s => s.position === position);
        if (index === -1) return;

        this.separators[index].element.remove();
        this.separators.splice(index, 1);
        this.updateSeparatorsTable();
        this.updateSeparatorsInfo();
        this.renderMiniNavigation();
        this.syncScrollPositions();
    }

    private autoSeparate(padding: number): void {
        if (padding < 4000 || padding > 14000) {
            this.showErrorModal("Відступ повинен бути між 4000 та 14000 пікселями");
            return;
        }

        let pos = padding;
        while (pos < this.totalHeight) {
            this.addSeparator(pos);
            pos += padding;
        }
    }

    private clearAllSeparators(): void {
        this.separators.forEach(separator => {
            separator.element.remove();
        });
        this.separators = [];
        this.updateSeparatorsTable();
        this.updateSeparatorsInfo();
        this.renderMiniNavigation();
        this.syncScrollPositions();
    }

    private updateSeparatorsTable(): void {
        const tableBody = document.querySelector('#separators-table tbody');
        if (!tableBody) return;

        tableBody.innerHTML = '';

        let prevPosition = 0;
        this.separators.forEach(separator => {
            const row = document.createElement('tr');
            
            const positionCell = document.createElement('td');
            const positionLink = document.createElement('a');
            positionLink.href = '#';
            positionLink.textContent = `${separator.position}px`;
            positionLink.addEventListener('click', (e) => {
                e.preventDefault();
                this.scrollToPosition(separator.position);
            });
            positionCell.appendChild(positionLink);
            
            const segmentCell = document.createElement('td');
            segmentCell.textContent = `${separator.position - prevPosition}px`;
            
            const actionCell = document.createElement('td');
            const deleteBtn = document.createElement('button');
            deleteBtn.textContent = 'Видалити';
            deleteBtn.className = 'btn danger';
            deleteBtn.addEventListener('click', () => {
                this.removeSeparator(separator.position);
            });
            actionCell.appendChild(deleteBtn);
            
            row.appendChild(positionCell);
            row.appendChild(segmentCell);
            row.appendChild(actionCell);
            
            tableBody.appendChild(row);
            prevPosition = separator.position;
        });
    }

    private updateSeparatorsInfo(): void {
        const infoElement = document.getElementById('separators-info');
        
        if (infoElement) {
            infoElement.textContent = `Розділювачів: ${this.separators.length} | Сторінок: ${this.separators.length + 1}`;
        }
    }

    private scrollToPosition(position: number): void {
        const imageViewer = document.getElementById('image-viewer');
        if (!imageViewer) return;

        // Center the position in the viewport
        const viewportHeight = imageViewer.clientHeight;
        const scrollTo = position - viewportHeight / 2;
        
        imageViewer.scrollTo({
            top: scrollTo,
            behavior: 'smooth'
        });
    }

    private async exportImages(): Promise<void> {
        if (this.separators.length === 0) {
            this.showErrorModal("Немає розділювачів для експорту");
            return;
        }

        if (!this.chapterPath) {
            this.showErrorModal("Шлях до папки не вказано");
            return;
        }

        this.updateProgress(0, "Експорт зображень...");
        
        let separator_positions: number[] = this.separators.map((separator) => {
            return separator.position;
        });

        console.log(separator_positions);
        

        await invoke("export_images", {
            separators: separator_positions,
            extention: "webp",
        });
    }
}

// Initialize the application
document.addEventListener('DOMContentLoaded', () => {
    new WebtoonProcessor();
});